use penumbra_mtk::Storage;
use penumbra_mtk::storage::{UfsInfo, UfsInfoV2, UfsStorage};
use penumbra_mtk::traits::FromBytes;

const UFS_XML_RESP: &'static str = include_str!("../files/storage/ufs_resp.xml");
const UFS_RESP_V2: &'static [u8] = include_bytes!("../files/storage/ufs_resp_v2.bin");

#[test]
fn test_ufs_valid_xml() {
    let ufs = UfsStorage::from_xml(UFS_XML_RESP).expect("UFS storage should parse successfully");

    assert_eq!(ufs.block_size(), 0x1000);
    assert_eq!(ufs.total_size(), 0x3B96800000);
    assert_eq!(ufs.get_pl1_size(), 0x400000);
    assert_eq!(ufs.get_pl2_size(), 0x400000);
    assert_eq!(ufs.get_user_size(), 0x3B96000000);
    assert_eq!(ufs.get_rpmb_size(), 0x0);
}

#[test]
fn test_ufs_invalid_xml() {
    let invalid_xml = r#"<ufs>
        <block_size>0x1000</block_size>
        <lua0_size>0x400000</lua0_size>
        <lua1_size>0x400000</lua1_size>
        <lua2_size>0x3B96000000</lua2_size>
        <lua3_size>0x0</lua3_size>
        <ufs_cid>0x1234567890ABCDEF</ufs_cid>
    </ufs>"#;

    let ufs = UfsStorage::from_xml(invalid_xml);

    assert!(ufs.is_err(), "UFS storage parsing should fail for invalid XML");
}

#[test]
fn test_ufs_valid_v2() {
    let ufs = UfsStorage::from_bytes(UFS_RESP_V2).expect("UFS storage should parse successfully");

    match ufs.info {
        UfsInfo::V2(ref info) => {
            let mut serial = [0u8; 132];
            serial[..12].copy_from_slice(b"DEADBEEF1234");
            assert_eq!(info.serial, serial, "Serial number should match expected value");
            let cid = b"WHY IS TIMI MAD?\0\0\0\0";
            assert_eq!(&info.cid, cid, "CID should match expected value");
        }
        _ => panic!("Expected UfsInfo::V2"),
    }

    assert_eq!(ufs.block_size(), 0x1000, "Block size should be 0x1000");
    assert_eq!(ufs.total_size(), 0x1DCB800000, "Total size should be 0x1DCB800000");
    assert_eq!(ufs.get_pl1_size(), 0x400000, "PL1 size should be 0x400000");
    assert_eq!(ufs.get_pl2_size(), 0x400000, "PL2 size should be 0x400000");
    assert_eq!(ufs.get_user_size(), 0x1DCB000000, "User size should be 0x1DCB000000");
    assert_eq!(ufs.get_rpmb_size(), 0, "RPMB size should be 0");
}

#[test]
fn test_ufs_invalid_v2() {
    let invalid_data = [0u8; 4];

    let ufs_v2 = UfsInfoV2::from_bytes(&invalid_data);
    assert!(ufs_v2.is_none(), "UFS V2 storage parsing should fail for invalid data");

    let ufs = UfsStorage::from_bytes(&invalid_data);
    assert!(ufs.is_none(), "UFS storage parsing should fail for invalid data");
}
