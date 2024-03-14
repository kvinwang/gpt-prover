#![cfg_attr(not(feature = "std"), no_std, no_main)]
//! This is a smart contract running on the Phala Phat Contract platform.
//! It provides a proof of code execution. When the user calls the `run_js` method and passes in a piece of JavaScript code,
//! the contract executes this code and outputs the execution result and the hash of the code as the result.

#[macro_use]
extern crate alloc;

#[ink::contract]
mod prover {
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;
    use phat_js::JsCode;
    use pink::{chain_extension::SigType, system::SystemRef};
    use serde::Serialize;
    use scale::{Decode, Encode};
    use ink::codegen::Env;

    use ink::storage::Mapping;

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

    #[derive(Encode, Decode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        #[codec(index = 1)]
        Unauthorized,
        #[codec(index = 2)]
        BadConfig,
        #[codec(index = 3)]
        JsError(String),
    }

    type Result<T, E=Error> = core::result::Result<T, E>;

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
    pub struct JsProver {
        owner: AccountId,
        secrets: Mapping<Hash, String>,
    }

    impl JsProver {
        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                owner: Self::env().caller(),
                secrets: Mapping::new(),
            }
        }
    }

    /// Queries the contract.
    impl JsProver {
        /// Returns the public key.
        #[ink(message)]
        pub fn pubkey(&self) -> Vec<u8> {
            pink::ext().get_public_key(SigType::Sr25519, &self.key())
        }

        /// Proves the output of a JavaScript code execution.
        ///
        /// # Arguments
        ///
        /// * `js_code` - The Javascript code to run.
        /// * `args` - The arguments to pass to the Javascript code.
        /// * `secret` - The secretData passed to the JavaScript code as global variable `secretData`.
        ///
        /// @ui js_code widget codemirror
        /// @ui js_code options.lang javascript
        #[ink(message)]
        pub fn run_js(
            &self,
            js_code: String,
            args: Vec<String>,
            secret: Option<String>,
        ) -> Result<ProvenOutput> {
            self.do_run_js(js_code, args, secret)
        }
    }

    /// Manages the contract owner's operations.
    impl JsProver {
        /// Transfer ownership to another account.
        #[ink(message)]
        pub fn transfer_ownership(&mut self, new_owner: AccountId) -> Result<()> {
            self.ensure_owner()?;
            self.owner = new_owner;
            Ok(())
        }

        /// Updates the secret data.
        #[ink(message)]
        pub fn set_secret(&mut self, for_code_hash: Hash, secret: String) -> Result<()> {
            self.ensure_owner()?;
            self.secrets.insert(for_code_hash, &secret);
            Ok(())
        }
    }

    impl JsProver {
        fn ensure_owner(&self) -> Result<()> {
            if self.env().caller() != self.owner {
                return Err(Error::Unauthorized);
            }
            Ok(())
        }

        fn key(&self) -> Vec<u8> {
            pink::ext().derive_sr25519_key(b"signer"[..].into())
        }

        fn secret_data(&self, code_hash: &Hash) -> String {
            self.secrets.get(code_hash).unwrap_or_default()
        }

        pub fn do_run_js(
            &self,
            js_code: String,
            args: Vec<String>,
            secret: Option<String>
        ) -> Result<ProvenOutput> {
            use phat_js as js;
            let js_code_hash: Hash = self
                .env()
                .hash_bytes::<ink::env::hash::Blake2x256>(js_code.as_bytes())
                .into();

            let mut args = args;
            let secret = match secret {
                Some(s) => s,
                None => self.secret_data(&js_code_hash),
            };
            args.push(secret);
            let init_script = "secretData = scriptArgs.pop();";

            let codes = vec![
                JsCode::Source(init_script.to_string()),
                JsCode::Source(js_code),
            ];
            let output = pink::ext().js_eval(codes, args);
            let output = match output {
                js::JsValue::String(s) => s,
                _ => return Err(Error::JsError(format!("Invalid output: {:?}", output))),
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
    }

    #[cfg(test)]
    mod tests {
        use super::JsProverRef;

        use alloc::vec;
        use pink_drink::{PinkRuntime, SessionExt, DeployBundle, Callable};
        use drink::session::Session;
        use ink::codegen::TraitCallBuilder;

        #[test]
        fn run_js_works() -> Result<(), Box<dyn std::error::Error>> {
            tracing_subscriber::fmt::init();
            const OPENAI_APIKEY: &str = env!("OPENAI_APIKEY");
            const OPENAI_URL: &str = "https://api.openai.com/v1/chat/completions";

            let contract_code = include_bytes!("./target/ink/js_prover.wasm");

            let mut session = Session::<PinkRuntime>::new()?;
            session.set_driver("JsRuntime", &[0u8; 32])?;

            let secret = format!(r#"{{
                "url": "{OPENAI_URL}",
                "apiKey": "{OPENAI_APIKEY}"
            }}"#);
            // Instantiate the contract.
            let mut contract_ref = JsProverRef::default()
                .deploy_wasm(contract_code, &mut session)?;

            let js_code = include_str!("./ask-gpt.js");
            // Set the secret data for ask-gpt.js.
            let js_code_hash = sp_core::blake2_256(js_code.as_bytes());
            contract_ref.call_mut().set_secret(js_code_hash.into(), secret).submit_tx(&mut session)?.unwrap();

            // Call the `run_js` method.
            let model = "gpt-3.5-turbo-0125".to_string();
            let prompt = "What is the meaning of life?".to_string();
            let result = contract_ref
                .call()
                .run_js(js_code.into(), vec![model, prompt], None)
                .query(&mut session)?;
            let output = result.unwrap().payload;
            println!("output: {}", output);

            // To be convenient, let's print the result using js
            let js_code = r#"
                const output = JSON.parse(JSON.parse(scriptArgs[0]).output);
                Sidevm.inspect('Output:', output);
                const reply = JSON.parse(output.reply);
                Sidevm.inspect('Reply:', reply);
            "#;
            let _result = contract_ref
                .call()
                .run_js(js_code.into(), vec![output], None)
                .query(&mut session)?;
            Ok(())
        }
    }
}
