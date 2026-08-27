use penumbra_mtk::da::{ScatterFile, ScatterOp, ScatterPartition};
use penumbra_mtk::storage::EmmcPartition;
use penumbra_mtk::{Partition, PartitionKind, StorageType};

const YAML_SCATTER: &'static str = include_str!("../files/da/MT6768_Android_scatter.txt");
const YAML_SCATTER_NEW: &'static str = include_str!("../files/da/MT6993_Android_scatter.txt");
const XML_SCATTER: &'static str = include_str!("../files/da/MT6768_Android_scatter.xml");
const XML_SCATTER_NEW: &'static str = include_str!("../files/da/MT6993_Android_scatter.xml");

#[test]
fn test_yaml_old_scatter() {
    ScatterFile::from_yaml(YAML_SCATTER).expect("Scatter file should be parsed successfully");
}

#[test]
fn test_yaml_new_scatter() {
    ScatterFile::from_yaml(YAML_SCATTER_NEW).expect("Scatter file should be parsed successfully");
}

#[test]
fn test_xml_old_scatter() {
    ScatterFile::from_xml(XML_SCATTER).expect("Scatter file should be parsed successfully");
}

#[test]
fn test_xml_new_scatter() {
    ScatterFile::from_xml(XML_SCATTER_NEW).expect("Scatter file should be parsed successfully");
}

#[test]
fn test_scatter_partition_flags() {
    let mut part = ScatterPartition::new(
        Partition {
            name: "komugi".into(),
            address: 0,
            size: 1024,
            kind: PartitionKind::Emmc(EmmcPartition::User),
        },
        None,
        ScatterOp::Protected,
        false,
        StorageType::Emmc,
        None,
    );

    assert!(part.is_protected());
    assert!(!part.is_invisible());
    assert!(!part.is_virtual());
    assert!(!part.need_resize());
    assert!(!part.is_reserved());

    part.op = ScatterOp::Invisible;
    assert!(part.is_invisible());

    part.op = ScatterOp::Logic;
    assert!(part.is_virtual());

    part.op = ScatterOp::NeedResize;
    assert!(part.need_resize());

    part.op = ScatterOp::Reserved;
    assert!(!part.is_reserved(), "Reserved should be false if address & 0xFFFF0000 != 0xFFFF0000");
}

#[test]
fn test_scatter_partition_reserved() {
    let mut part = ScatterPartition::new(
        Partition {
            name: "komaru".into(),
            address: 0xFFFF0000,
            size: 1024,
            kind: PartitionKind::Emmc(EmmcPartition::User),
        },
        None,
        ScatterOp::Invisible,
        false,
        StorageType::Emmc,
        None,
    );

    assert!(!part.is_reserved(), "Reserved should evaluate to false if op is not Reserved");

    part.op = ScatterOp::Reserved;

    assert!(
        part.is_reserved(),
        "Reserved should evaluate to true if op is Reserved and address & 0xFFFF0000 == 0xFFFF0000"
    );
}
