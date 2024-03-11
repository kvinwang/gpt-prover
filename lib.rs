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
    use pink::{chain_extension::SigType, system::SystemRef, ConvertTo};
    use scale::{Decode, Encode};

    #[derive(Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    /// Struct representing the signed payload.
    pub struct ProvenPayload {
        pub js_output: String,
        pub js_code_hash: Hash,
        pub js_engine_code_hash: Hash,
        pub contract_code_hash: Hash,
        pub contract_address: AccountId,
        pub block_number: u32,
    }

    #[derive(Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    /// Struct representing the output of a proven execution.
    pub struct ProvenOutput {
        pub payload: ProvenPayload,
        pub signature: Vec<u8>,
        pub signing_pubkey: Vec<u8>,
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
            let js_code_hash = self
                .env()
                .hash_bytes::<ink::env::hash::Blake2x256>(js_code.as_bytes())
                .into();
            let output = js::eval_async_js(&js_code, &args);
            let js_output = match output {
                js::JsValue::String(s) => s,
                _ => return Err(format!("Invalid output: {:?}", output)),
            };
            let key = self.key();
            let driver = SystemRef::instance()
                .get_driver("JsRuntime".into())
                .expect("Failed to get Js driver");
            let payload = ProvenPayload {
                js_code_hash,
                js_engine_code_hash: driver.convert_to(),
                contract_code_hash: self
                    .env()
                    .own_code_hash()
                    .expect("Failed to get contract code hash"),
                contract_address: self.env().account_id(),
                js_output,
                block_number: self.env().block_number(),
            };
            let signature = pink::ext().sign(SigType::Sr25519, &key, &payload.encode());
            Ok(ProvenOutput {
                payload,
                signature,
                signing_pubkey: self.pubkey(),
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
}
