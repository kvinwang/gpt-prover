#![cfg_attr(not(feature = "std"), no_std, no_main)]
//! This is a smart contract running on the Phala Phat Contract platform.
//! It provides a proof of code execution. When the user calls the `prove_output` method and passes in a piece of JavaScript code,
//! the contract executes this code and outputs the execution result and the hash of the code as the result.

#[macro_use]
extern crate alloc;

#[ink::contract]
mod gpt_prover {
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

    #[derive(Encode, Decode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        #[codec(index = 1)]
        Unauthorized,
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
    pub struct GptProver {
        owner: AccountId,
        api_url: String,
        api_key: String,
    }

    impl GptProver {
        #[ink(constructor)]
        pub fn new(api_url: String, api_key: String) -> Self {
            Self {
                owner: Self::env().caller(),
                api_url,
                api_key,
            }
        }
    }

    /// Queries the contract.
    impl GptProver {
        #[ink(message)]
        /// Returns the public key.
        pub fn pubkey(&self) -> Vec<u8> {
            pink::ext().get_public_key(SigType::Sr25519, &self.key())
        }

        #[ink(message)]
        /// Returns the current API URL.
        pub fn api_url(&self) -> String {
            self.api_url.clone()
        }

        #[ink(message)]
        /// Ask given model a question.
        pub fn ask_gpt(&self, model: String, prompt: String) -> Result<ProvenOutput, String> {
            self.ask_openai(&model, &prompt)
        }

        #[ink(message)]
        /// Ask GPT-4 a question.
        pub fn ask_gpt4(&self, prompt: String) -> Result<ProvenOutput, String> {
            self.ask_openai("gpt-4-turbo-preview", &prompt)
        }

        #[ink(message)]
        /// Ask GPT-4 a question.
        pub fn ask_gpt3n5(&self, prompt: String) -> Result<ProvenOutput, String> {
            self.ask_openai("gpt-3.5-turbo-0125", &prompt)
        }
    }

    /// Manages the contract owner's operations.
    impl GptProver {
        #[ink(message)]
        /// Updates the API URL.
        pub fn update_api_url(&mut self, new_url: String) -> Result<()> {
            self.ensure_owner()?;
            self.api_url = new_url;
            Ok(())
        }

        #[ink(message)]
        /// Updates the API key.
        pub fn update_api_key(&mut self, new_key: String) -> Result<()> {
            self.ensure_owner()?;
            self.api_key = new_key;
            Ok(())
        }

        #[ink(message)]
        /// Transfer ownership to another account.
        pub fn transfer_ownership(&mut self, new_owner: AccountId) -> Result<()> {
            self.ensure_owner()?;
            self.owner = new_owner;
            Ok(())
        }

    }

    use ink::codegen::Env;
    impl GptProver {
        fn ensure_owner(&self) -> Result<()> {
            if self.env().caller() != self.owner {
                return Err(Error::Unauthorized.into());
            }
            Ok(())
        }

        fn key(&self) -> Vec<u8> {
            pink::ext().derive_sr25519_key(b"signer"[..].into())
        }

        fn ask_openai(&self, model: &str, prompt: &str) -> Result<ProvenOutput, String> {
            const JS: &str = include_str!("askgpt.js");
            self.run_js(JS, alloc::vec![self.api_url.clone(), self.api_key.clone(), model.into(), prompt.into()])
        }

        fn run_js(
            &self,
            js_code: &str,
            args: Vec<String>,
        ) -> Result<ProvenOutput, String> {
            use phat_js as js;
            let js_code_hash: Hash = self
                .env()
                .hash_bytes::<ink::env::hash::Blake2x256>(js_code.as_bytes())
                .into();
            let output = js::eval_async_js(js_code, &args);
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
    }

    #[cfg(test)]
    mod tests {
        use super::GptProverRef;

        use pink_drink::{PinkRuntime, SessionExt, DeployBundle, Callable};
        use drink::session::Session;
        use ink::codegen::TraitCallBuilder;

        #[test]
        fn run_js_works() -> Result<(), Box<dyn std::error::Error>> {
            const OPENAI_APIKEY: &str = env!("OPENAI_APIKEY");
            const OPENAI_URL: &str = "https://api.openai.com/v1/chat/completions";

            let mut session = Session::<PinkRuntime>::new()?;
            session.set_driver("JsRuntime", &[0u8; 32])?;

            let wasm = include_bytes!("./target/ink/proven.wasm").to_vec();
            let contract_ref = GptProverRef::new(OPENAI_URL.into(), OPENAI_APIKEY.into())
                .deploy_wasm(&wasm, &mut session)?;

            let result = contract_ref.call().ask_gpt3n5("What is asdf?".into()).query(&mut session)?;
            println!("payload: {}", result.unwrap().payload);
            Ok(())
        }
    }
}
