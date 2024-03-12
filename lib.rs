#![cfg_attr(not(feature = "std"), no_std, no_main)]
//! This is a smart contract running on the Phala Phat Contract platform.
//! It provides a proof of code execution. When the user calls the `prove_output` method and passes in a piece of JavaScript code,
//! the contract executes this code and outputs the execution result and the hash of the code as the result.

#[macro_use]
extern crate alloc;

#[ink::contract]
mod proven {
    use alloc::string::String;
    use alloc::vec::Vec;
    use pink::{chain_extension::SigType, system::SystemRef};
    use serde::Serialize;
    use scale::{Decode, Encode};

    struct Hexed<T>(T);

    impl<T> From<T> for Hexed<T> {
        fn from(t: T) -> Self {
            Hexed(t)
        }
    }

    impl<T: AsRef<[u8]>> Serialize for Hexed<T> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&format!("0x{}", hex::encode(self.0.as_ref())))
        }
    }

    #[derive(Serialize)]
    /// Struct representing the signed payload.
    pub struct ProvenPayload {
        output: String,
        js_code_hash: Hexed<Hash>,
        js_engine_code_hash: Hexed<AccountId>,
        contract_code_hash: Hexed<Hash>,
        contract_address: Hexed<AccountId>,
        block_number: u32,
    }

    #[derive(Encode, Decode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    /// Struct representing the output of a proven execution.
    pub struct ProvenOutput {
        payload: String,
        signature: Vec<u8>,
        pubkey: Vec<u8>,
    }

    #[ink(storage)]
    pub struct Proven {}

    impl Proven {
        #[ink(constructor)]
        pub fn default() -> Self {
            Self {}
        }

        #[ink(message)]
        /// Returns the public key.
        pub fn pubkey(&self) -> Vec<u8> {
            pink::ext().get_public_key(SigType::Sr25519, &self.key())
        }

        #[ink(message)]
        /// Executes the provided JavaScript code and returns the execution result and the hash of the code.
        /// The output is signed with dedicated private key.
        ///
        /// # Arguments
        ///
        /// * `js_code` - The Javascript code to run
        /// * `args` - The arguments to pass to the Javascript code
        ///
        /// @ui js_code widget codemirror
        /// @ui js_code options.lang javascript
        pub fn run_js(
            &self,
            js_code: String,
            args: Vec<String>,
        ) -> Result<ProvenOutput, String> {
            use phat_js as js;
            let js_code_hash: Hash = self
                .env()
                .hash_bytes::<ink::env::hash::Blake2x256>(js_code.as_bytes())
                .into();
            let output = js::eval_async_js(&js_code, &args);
            let output = match output {
                js::JsValue::String(s) => s,
                _ => return Err(format!("Invalid output: {:?}", output)),
            };
            let key = self.key();
            let driver = SystemRef::instance()
                .get_driver("JsRuntime".into())
                .expect("Failed to get Js driver");
            let payload = ProvenPayload {
                js_code_hash: js_code_hash.into(),
                js_engine_code_hash: driver.into(),
                contract_code_hash: self
                    .env()
                    .own_code_hash()
                    .expect("Failed to get contract code hash").into(),
                contract_address: self.env().account_id().into(),
                block_number: self.env().block_number(),
                output,
            };
            let payload_str = pink_json::to_string(&payload).expect("Failed to serialize payload");
            let signature = pink::ext().sign(SigType::Sr25519, &key, &payload_str.as_bytes());
            Ok(ProvenOutput {
                payload: payload_str,
                signature,
                pubkey: self.pubkey(),
            })
        }

        #[ink(message)]
        /// Same as run_js except getting the code from given URL.
        pub fn run_js_from_url(
            &self,
            code_url: String,
            args: Vec<String>,
        ) -> Result<ProvenOutput, String> {
            let response = pink::http_get!(
                code_url,
                alloc::vec![("User-Agent".into(), "phat-contract".into())]
            );
            if (response.status_code / 100) != 2 {
                return Err("Failed to get code".into());
            }
            let js_code = String::from_utf8(response.body).map_err(|_| "Invalid code")?;
            self.run_js(js_code, args)
        }
    }

    impl Proven {
        /// Returns the key used to sign the execution result.
        fn key(&self) -> Vec<u8> {
            pink::ext().derive_sr25519_key(b"signer"[..].into())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::ProvenRef;

        use pink_drink::{PinkRuntime, SessionExt, DeployBundle, Callable};
        use drink::session::Session;
        use ink::codegen::TraitCallBuilder;

        #[test]
        fn run_js_works() -> Result<(), Box<dyn std::error::Error>> {
            tracing_subscriber::fmt::init();
            let mut session = Session::<PinkRuntime>::new()?;
            session.set_driver("JsRuntime", &[0u8; 32])?;
            let wasm = include_bytes!("./target/ink/proven.wasm").to_vec();
            let contract_ref = ProvenRef::default().deploy_wasm(&wasm, &mut session)?;
            let result = contract_ref.call().run_js("\"Hello\"".into(), vec![]).query(&mut session)?;
            println!("payload: {}", result.unwrap().payload);
            Ok(())
        }
    }
}
