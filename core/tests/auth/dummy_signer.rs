use std::sync::Arc;

use penumbra_mtk::{AuthManager, SignPurpose, SignRequest, Signer};

struct DummySigner;

impl Signer for DummySigner {
    fn can_handle(&self, _pubk_mod: &[u8]) -> bool {
        true
    }

    fn is_authorized(&self, _req: &SignRequest) -> bool {
        true
    }

    fn sign(&self, _req: &SignRequest) -> penumbra_mtk::Result<Vec<u8>> {
        return Ok(vec![0u8; 256]);
    }
}

fn setup_dummy_signer() {
    let auth = AuthManager::get();
    let signer = DummySigner;
    auth.register_signer(Arc::new(signer)).expect("Signer should be registered successfully");
}

#[test]
fn test_dummy_signer() {
    setup_dummy_signer();

    let auth = AuthManager::get();
    let pubk_mod = vec![0u8; 256];
    let sign_data = penumbra_mtk::SignData { raw: vec![1, 2, 3], ..Default::default() };
    let req =
        SignRequest { pubk_mod: pubk_mod.clone(), data: sign_data, purpose: SignPurpose::BromSla };

    assert!(auth.can_sign(&pubk_mod), "Dummy signer should be able to sign");

    let data = auth.sign(&req).expect("Signing should succeed with dummy signer");

    let expected_data = vec![0u8; 256];
    assert_eq!(data, expected_data, "Signed data should match expected dummy signature");
}
