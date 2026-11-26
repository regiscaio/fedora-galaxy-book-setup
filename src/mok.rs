use crate::{
    AKMODS_PUBLIC_CERTIFICATE_STATE_PATH as CERT, AkmodsKeyIdentity, normalize_colonized_sha1,
    probe_output, strict_success_stdout,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Enrollment {
    Enrolled,
    AwaitingEnrollment,
    NotEnrolled,
    Unknown,
}

// Only unindented record fields from the Fedora mokutil long listing are
// authoritative. In particular a CN or embedded text inside the X.509 dump
// cannot manufacture a new fingerprint field or hide stdout diagnostics.
fn fingerprints(list: &str) -> Option<Vec<String>> {
    if list.is_empty() {
        return Some(Vec::new());
    }
    if list.chars().any(|c| c.is_control() && c != '\n') {
        return None;
    }
    let mut keys = Vec::new();
    for (i, record) in list.strip_suffix('\n')?.split("\n\n").enumerate() {
        let mut lines = record.split('\n');
        if lines.next()? != format!("[key {}]", i + 1) {
            return None;
        }
        let owner = lines.next()?.strip_prefix("Owner: ")?;
        if owner.len() != 36
            || !owner.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit()
                }
            })
        {
            return None;
        }
        let key = normalize_colonized_sha1(lines.next()?.strip_prefix("SHA1 Fingerprint: ")?)?;
        if lines.next()? != "Certificate:" || lines.next()? != "    Data:" {
            return None;
        }
        let mut body = lines.peekable();
        body.next_if(|s| s.starts_with("        "))?;
        while body.next_if(|s| s.starts_with("        ")).is_some() {}
        if body
            .next()?
            .strip_prefix("    Signature Algorithm: ")?
            .is_empty()
            || body.next()? != "    Signature Value:"
        {
            return None;
        }
        // The direct DER test provides the cryptographic check. OpenSSL's
        // indented display of the signature is opaque and algorithm-specific.
        body.peek()?;
        if !body.all(|line| {
            line.strip_prefix("        ")
                .is_some_and(|s| !s.trim().is_empty())
        }) {
            return None;
        }
        keys.push(key);
    }
    Some(keys)
}

fn classify(
    identity: &AkmodsKeyIdentity,
    test: Option<&std::process::Output>,
    enrolled: Option<&str>,
    additions: Option<&str>,
    deletions: Option<&str>,
) -> Enrollment {
    let Some(test) = test else {
        return Enrollment::Unknown;
    };
    if !matches!(test.status.code(), Some(0 | 1)) || !test.stderr.is_empty() {
        return Enrollment::Unknown;
    }
    let (Some(list), Some(add), Some(del)) = (
        enrolled.and_then(fingerprints),
        additions.and_then(fingerprints),
        deletions.and_then(fingerprints),
    ) else {
        return Enrollment::Unknown;
    };
    if !del.is_empty() {
        return Enrollment::Unknown;
    }
    let present = list.contains(&identity.fingerprint_sha1);
    let answer = |message: &str| test.stdout == format!("{CERT} {message}\n").as_bytes();
    if !present
        && add.as_slice() == [identity.fingerprint_sha1.as_str()]
        && answer("is already in the enrollment request")
    {
        return Enrollment::AwaitingEnrollment;
    }
    if add.is_empty() {
        if present && answer("is already enrolled") {
            return Enrollment::Enrolled;
        }
        if !present && answer("is not enrolled") {
            return Enrollment::NotEnrolled;
        }
    }
    Enrollment::Unknown
}

pub(super) fn query(identity: &AkmodsKeyIdentity) -> Enrollment {
    const MOKUTIL: &str = "/usr/bin/mokutil";
    let test = probe_output(MOKUTIL, &["--ignore-keyring", "--test-key", CERT], true).ok();
    let list = |flag: &str| probe_output(MOKUTIL, &[flag], true).ok();
    let enrolled = list("--list-enrolled");
    let added = list("--list-new");
    let deleted = list("--list-delete");
    classify(
        identity,
        test.as_ref(),
        enrolled.as_ref().and_then(strict_success_stdout),
        added.as_ref().and_then(strict_success_stdout),
        deleted.as_ref().and_then(strict_success_stdout),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::process::ExitStatusExt,
        process::{ExitStatus, Output},
    };

    const OURS: &str = "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33";
    const OTHER: &str = "FF:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33";

    fn identity(fp: &str) -> AkmodsKeyIdentity {
        AkmodsKeyIdentity {
            cn: "same-cn".into(),
            fingerprint_sha1: fp.into(),
            certificate_serial: "1122".into(),
        }
    }
    fn record(index: usize, fp: &str) -> String {
        format!(
            "[key {index}]\nOwner: 00000000-1111-2222-3333-444444444444\nSHA1 Fingerprint: {fp}\nCertificate:\n    Data:\n        Subject: CN=same-cn\n    Signature Algorithm: sha256WithRSAEncryption\n    Signature Value:\n        00:01:\n        02:03\n"
        )
    }
    fn answer(status: i32, message: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(status << 8),
            stdout: format!("{CERT} {message}\n").into_bytes(),
            stderr: vec![],
        }
    }
    fn observed(out: &Output, enrolled: &str, add: &str, del: &str) -> Enrollment {
        classify(
            &identity(OURS),
            Some(out),
            Some(enrolled),
            Some(add),
            Some(del),
        )
    }

    #[test]
    fn enrolled_needs_both_proofs_not_status_or_cn() {
        let ours = record(1, OURS);
        let other = record(1, OTHER);
        for status in [0, 1] {
            let yes = answer(status, "is already enrolled");
            let no = answer(status, "is not enrolled");
            assert_eq!(observed(&yes, &ours, "", ""), Enrollment::Enrolled);
            assert_eq!(observed(&yes, &other, "", ""), Enrollment::Unknown);
            assert_eq!(observed(&no, &ours, "", ""), Enrollment::Unknown);
            assert_eq!(observed(&no, &other, "", ""), Enrollment::NotEnrolled);
        }
        assert_eq!(
            crate::validated_key_identity(Some(identity(OURS)), None),
            None
        );
        assert_eq!(
            crate::validated_key_identity(Some(identity(OURS)), Some(identity(OTHER))),
            None
        );
        assert_eq!(
            crate::validated_key_identity(Some(identity(OURS)), Some(identity(OURS))),
            Some(identity(OURS))
        );
    }

    #[test]
    fn queues_require_matching_request_and_no_conflicts() {
        let ours = record(1, OURS);
        let other = record(1, OTHER);
        let pending = answer(1, "is already in the enrollment request");
        assert_eq!(
            observed(&pending, &other, &ours, ""),
            Enrollment::AwaitingEnrollment
        );
        for (enrolled, add, del) in [
            (&ours[..], &ours[..], ""),
            (&other, "", ""),
            ("", &other, ""),
            ("", &ours, &other),
        ] {
            assert_eq!(observed(&pending, enrolled, add, del), Enrollment::Unknown);
        }
        let yes = answer(1, "is already enrolled");
        assert_eq!(observed(&yes, &ours, &other, ""), Enrollment::Unknown);
        assert_eq!(observed(&yes, &ours, "", &ours), Enrollment::Unknown);
    }

    #[test]
    fn rejection_errors_and_malformed_output_never_authorize() {
        let ours = record(1, OURS);
        let yes = answer(1, "is already enrolled");
        for bad in [
            format!("{ours}warning\n"),
            ours.replace("SHA1 Fingerprint:", "Subject: SHA1 Fingerprint:"),
            ours.replace("    Signature Value:\n", ""),
            ours.replace("        00:01:\n        02:03\n", ""),
            ours.replace("Subject:", "Subject:\r"),
        ] {
            assert_eq!(observed(&yes, &bad, "", ""), Enrollment::Unknown);
        }
        let mut noise = yes.clone();
        noise.stderr.push(b'!');
        let mut wrong_path = yes.clone();
        wrong_path.stdout = b"other.der is already enrolled\n".to_vec();
        for bad in [
            noise,
            wrong_path,
            answer(255, "is already enrolled"),
            answer(0, "is blocked"),
        ] {
            assert_eq!(observed(&bad, &ours, "", ""), Enrollment::Unknown);
        }
        assert_eq!(
            classify(&identity(OURS), Some(&yes), None, Some(""), Some("")),
            Enrollment::Unknown
        );
        assert_eq!(
            classify(&identity(OURS), Some(&yes), Some(&ours), None, Some("")),
            Enrollment::Unknown
        );
    }

    #[test]
    fn listing_allows_another_certificate_signature_format() {
        let other = record(1, OTHER)
            .replace("sha256WithRSAEncryption", "ecdsa-with-SHA384")
            .replace("00:01:\n        02:03", "r: 00:01\n        s: 02:03");
        let list = format!("{other}\n{}", record(2, OURS));
        assert_eq!(
            observed(&answer(1, "is already enrolled"), &list, "", ""),
            Enrollment::Enrolled
        );
    }

    #[test]
    #[ignore = "requires a Fedora 44 host with an enrolled public certificate; read only"]
    fn real_host_listing_uses_supported_records() {
        let output = probe_output("/usr/bin/mokutil", &["--list-enrolled"], true).unwrap();
        let fields = fingerprints(strict_success_stdout(&output).unwrap()).unwrap();
        assert!(!fields.is_empty());
    }
}
