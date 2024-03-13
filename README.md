# JS Prover

The JS Prover is an adaptable contract designed for proving the legitimacy of outputs derived from JS code executions. This contract is especially beneficial when integrating with the AI model to verify the accuracy of generated outputs. It operates under two distinct modes: Public and Whitelisted, accommodating a broad range of use cases from open execution environments to more secure, restricted ones.

## Design Overview
The evolution from the GptProver to the JS Prover represents a shift towards increased flexibility. The contract accepts JS code as input, executing it to produce a verifiable, signed output.

### Modes of Operation

#### Public Mode
- **Unrestricted Access:** Any JS code can be executed without limitations.
- **No Secret Information:** Ensures no confidential data (e.g., API keys) is passed to the JS script during execution.

#### Whitelisted Mode
- **Execution Control:** Only pre-approved JS scripts are permitted to run, providing an additional layer of security.
- **Owner Privileges:**
  - **Whitelist Management:** The contract owner has the authority to manage which scripts are allowed.
  - **Confidential Data Handling:** The owner can configure the contract to pass secret data (e.g., API keys) as arguments (`arg0`) to the whitelisted JS scripts, securing sensitive information.

## Usage Instructions

Please refer to the provided test cases.
