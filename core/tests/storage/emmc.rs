use penumbra_mtk::Storage;
use penumbra_mtk::storage::EmmcStorage;
use penumbra_mtk::traits::FromBytes;

const EMMC_RESP: &[u8] = include_bytes!("../files/storage/emmc_resp.bin");

#[test]
fn test_emmc_valid() {
    let ufs = EmmcStorage::from_bytes(EMMC_RESP).expect("EMMC storage should parse successfully");

    assert_eq!(ufs.block_size(), 0x1000);
    assert_eq!(ufs.total_size(), 0x3A4A00000);
    assert_eq!(ufs.get_pl1_size(), 0x400000);
    assert_eq!(ufs.get_pl2_size(), 0x400000);
    assert_eq!(ufs.get_user_size(), 0x3A3E00000);
    assert_eq!(ufs.get_rpmb_size(), 0x400000);
}

#[test]
fn test_emmc_invalid() {
    let invalid_data = [0u8; 4];

    let emmc = EmmcStorage::from_bytes(&invalid_data);

    assert!(emmc.is_none(), "EMMC storage parsing should fail for invalid data");
}
