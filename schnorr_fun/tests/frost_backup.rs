#![cfg(feature = "frost_backup")]
use core::str::FromStr;
use schnorr_fun::frost_backup::{self, decode_backup, encode_backup, polynomial_identifier};
use secp256kfun::{g, marker::Secret, Scalar, G};

#[test]
fn frost_backup_short() {
    let threshold = 4;
    let polynomial = vec![g!(1 * G).normalize()];
    let secret_share = Scalar::<Secret>::from_str(
        "1234123412341234123412341234123412341234123412341234123412341234",
    )
    .unwrap();
    let share_index = frost_backup::BackupShareIndex::SmallIndex(7);

    let frost_backup = encode_backup::<sha2::Sha256>(
        threshold,
        polynomial.clone(),
        secret_share.clone(),
        share_index.clone(),
    )
    .unwrap();
    dbg!(&frost_backup);

    let (decoded_threshold, decoded_identifier, decoded_secret_share, decoded_share_index) =
        decode_backup(frost_backup).unwrap();

    assert_eq!(threshold, decoded_threshold);
    assert_eq!(
        polynomial_identifier::<sha2::Sha256>(polynomial),
        decoded_identifier
    );
    assert_eq!(secret_share, decoded_secret_share);
    assert_eq!(share_index, decoded_share_index);
}

#[test]
fn frost_backup_long() {
    let threshold = 31;
    let polynomial = vec![
        g!(1 * G).normalize(),
        g!(2 * G).normalize(),
        g!(3 * G).normalize(),
    ]; // some polynomial coefficients
    let secret_share = Scalar::<Secret>::from_str(
        "7373737373737373737373737373737373737373737373737373737373737373",
    )
    .unwrap();
    let share_index = frost_backup::BackupShareIndex::Scalar(
        Scalar::<Secret>::from_str(
            "34f7ce653cfa8454b3463726a599ef2925736442d2d06455974d6feae9450d90",
        )
        .unwrap(),
    );

    let frost_backup = encode_backup::<sha2::Sha256>(
        threshold,
        polynomial.clone(),
        secret_share.clone(),
        share_index.clone(),
    )
    .unwrap();
    dbg!(&frost_backup);

    let (decoded_threshold, decoded_identifier, decoded_secret_share, decoded_share_index) =
        decode_backup(frost_backup).unwrap();

    assert_eq!(threshold, decoded_threshold);
    assert_eq!(
        polynomial_identifier::<sha2::Sha256>(polynomial),
        decoded_identifier
    );
    assert_eq!(secret_share, decoded_secret_share);
    assert_eq!(share_index, decoded_share_index);
}
