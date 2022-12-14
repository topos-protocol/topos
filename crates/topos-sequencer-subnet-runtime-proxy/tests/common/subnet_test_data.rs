use std::io::Write;

pub const TEST_KEYSTORE_FILE_NAME: &str = "test-keystore.json";
pub const TEST_KEYSTORE_FILE_PASSWORD: &str = "123";

pub const SUBNET_ERC20_CONTRACT_ABI: &str = r#"[
    {
      "inputs": [
        {
          "internalType": "string",
          "name": "name_",
          "type": "string"
        },
        {
          "internalType": "string",
          "name": "symbol_",
          "type": "string"
        }
      ],
      "stateMutability": "nonpayable",
      "type": "constructor"
    },
    {
      "anonymous": false,
      "inputs": [
        {
          "indexed": true,
          "internalType": "address",
          "name": "owner",
          "type": "address"
        },
        {
          "indexed": true,
          "internalType": "address",
          "name": "spender",
          "type": "address"
        },
        {
          "indexed": false,
          "internalType": "uint256",
          "name": "value",
          "type": "uint256"
        }
      ],
      "name": "Approval",
      "type": "event"
    },
    {
      "anonymous": false,
      "inputs": [
        {
          "indexed": true,
          "internalType": "address",
          "name": "previousOwner",
          "type": "address"
        },
        {
          "indexed": true,
          "internalType": "address",
          "name": "newOwner",
          "type": "address"
        }
      ],
      "name": "OwnershipTransferred",
      "type": "event"
    },
    {
      "anonymous": false,
      "inputs": [
        {
          "indexed": true,
          "internalType": "address",
          "name": "from",
          "type": "address"
        },
        {
          "indexed": true,
          "internalType": "address",
          "name": "to",
          "type": "address"
        },
        {
          "indexed": false,
          "internalType": "uint256",
          "name": "value",
          "type": "uint256"
        }
      ],
      "name": "Transfer",
      "type": "event"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "owner",
          "type": "address"
        },
        {
          "internalType": "address",
          "name": "spender",
          "type": "address"
        }
      ],
      "name": "allowance",
      "outputs": [
        {
          "internalType": "uint256",
          "name": "",
          "type": "uint256"
        }
      ],
      "stateMutability": "view",
      "type": "function"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "spender",
          "type": "address"
        },
        {
          "internalType": "uint256",
          "name": "amount",
          "type": "uint256"
        }
      ],
      "name": "approve",
      "outputs": [
        {
          "internalType": "bool",
          "name": "",
          "type": "bool"
        }
      ],
      "stateMutability": "nonpayable",
      "type": "function"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "account",
          "type": "address"
        }
      ],
      "name": "balanceOf",
      "outputs": [
        {
          "internalType": "uint256",
          "name": "",
          "type": "uint256"
        }
      ],
      "stateMutability": "view",
      "type": "function"
    },
    {
      "inputs": [],
      "name": "decimals",
      "outputs": [
        {
          "internalType": "uint8",
          "name": "",
          "type": "uint8"
        }
      ],
      "stateMutability": "view",
      "type": "function"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "spender",
          "type": "address"
        },
        {
          "internalType": "uint256",
          "name": "subtractedValue",
          "type": "uint256"
        }
      ],
      "name": "decreaseAllowance",
      "outputs": [
        {
          "internalType": "bool",
          "name": "",
          "type": "bool"
        }
      ],
      "stateMutability": "nonpayable",
      "type": "function"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "spender",
          "type": "address"
        },
        {
          "internalType": "uint256",
          "name": "addedValue",
          "type": "uint256"
        }
      ],
      "name": "increaseAllowance",
      "outputs": [
        {
          "internalType": "bool",
          "name": "",
          "type": "bool"
        }
      ],
      "stateMutability": "nonpayable",
      "type": "function"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "account",
          "type": "address"
        },
        {
          "internalType": "uint256",
          "name": "amount",
          "type": "uint256"
        }
      ],
      "name": "mint",
      "outputs": [],
      "stateMutability": "nonpayable",
      "type": "function"
    },
    {
      "inputs": [],
      "name": "name",
      "outputs": [
        {
          "internalType": "string",
          "name": "",
          "type": "string"
        }
      ],
      "stateMutability": "view",
      "type": "function"
    },
    {
      "inputs": [],
      "name": "owner",
      "outputs": [
        {
          "internalType": "address",
          "name": "",
          "type": "address"
        }
      ],
      "stateMutability": "view",
      "type": "function"
    },
    {
      "inputs": [],
      "name": "renounceOwnership",
      "outputs": [],
      "stateMutability": "nonpayable",
      "type": "function"
    },
    {
      "inputs": [],
      "name": "symbol",
      "outputs": [
        {
          "internalType": "string",
          "name": "",
          "type": "string"
        }
      ],
      "stateMutability": "view",
      "type": "function"
    },
    {
      "inputs": [],
      "name": "totalSupply",
      "outputs": [
        {
          "internalType": "uint256",
          "name": "",
          "type": "uint256"
        }
      ],
      "stateMutability": "view",
      "type": "function"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "to",
          "type": "address"
        },
        {
          "internalType": "uint256",
          "name": "amount",
          "type": "uint256"
        }
      ],
      "name": "transfer",
      "outputs": [
        {
          "internalType": "bool",
          "name": "",
          "type": "bool"
        }
      ],
      "stateMutability": "nonpayable",
      "type": "function"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "from",
          "type": "address"
        },
        {
          "internalType": "address",
          "name": "to",
          "type": "address"
        },
        {
          "internalType": "uint256",
          "name": "amount",
          "type": "uint256"
        }
      ],
      "name": "transferFrom",
      "outputs": [
        {
          "internalType": "bool",
          "name": "",
          "type": "bool"
        }
      ],
      "stateMutability": "nonpayable",
      "type": "function"
    },
    {
      "inputs": [
        {
          "internalType": "address",
          "name": "owner",
          "type": "address"
        }
      ],
      "name": "transferOwnership",
      "outputs": [],
      "stateMutability": "nonpayable",
      "type": "function"
    }
  ]"#;
pub const SUBNET_ERC20_CONTRACT_CODE: &str = "60806040526c100000000000000000000000006006553480156200002257600080fd5b5060405162000ed538038062000ed583398101604081905262000045916200026c565b62000050336200008a565b60046200005e838262000364565b5060056200006d828262000364565b506200008233600654620000da60201b60201c565b505062000458565b600080546001600160a01b038381166001600160a01b0319831681178455604051919092169283917f8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e09190a35050565b6001600160a01b038216620001355760405162461bcd60e51b815260206004820152601f60248201527f45524332303a206d696e7420746f20746865207a65726f206164647265737300604482015260640160405180910390fd5b806003600082825462000149919062000430565b90915550506001600160a01b0382166000818152600160209081526040808320805486019055518481527fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef910160405180910390a35050565b505050565b634e487b7160e01b600052604160045260246000fd5b600082601f830112620001cf57600080fd5b81516001600160401b0380821115620001ec57620001ec620001a7565b604051601f8301601f19908116603f01168101908282118183101715620002175762000217620001a7565b816040528381526020925086838588010111156200023457600080fd5b600091505b8382101562000258578582018301518183018401529082019062000239565b600093810190920192909252949350505050565b600080604083850312156200028057600080fd5b82516001600160401b03808211156200029857600080fd5b620002a686838701620001bd565b93506020850151915080821115620002bd57600080fd5b50620002cc85828601620001bd565b9150509250929050565b600181811c90821680620002eb57607f821691505b6020821081036200030c57634e487b7160e01b600052602260045260246000fd5b50919050565b601f821115620001a257600081815260208120601f850160051c810160208610156200033b5750805b601f850160051c820191505b818110156200035c5782815560010162000347565b505050505050565b81516001600160401b03811115620003805762000380620001a7565b6200039881620003918454620002d6565b8462000312565b602080601f831160018114620003d05760008415620003b75750858301515b600019600386901b1c1916600185901b1785556200035c565b600085815260208120601f198616915b828110156200040157888601518255948401946001909101908401620003e0565b5085821015620004205787850151600019600388901b60f8161c191681555b5050505050600190811b01905550565b808201808211156200045257634e487b7160e01b600052601160045260246000fd5b92915050565b610a6d80620004686000396000f3fe608060405234801561001057600080fd5b50600436106100f55760003560e01c806370a0823111610097578063a457c2d711610066578063a457c2d7146101eb578063a9059cbb146101fe578063dd62ed3e14610211578063f2fde38b1461022457600080fd5b806370a0823114610197578063715018a6146101c05780638da5cb5b146101c857806395d89b41146101e357600080fd5b806323b872dd116100d357806323b872dd1461014d578063313ce56714610160578063395093511461016f57806340c10f191461018257600080fd5b806306fdde03146100fa578063095ea7b31461011857806318160ddd1461013b575b600080fd5b610102610235565b60405161010f91906108b7565b60405180910390f35b61012b610126366004610921565b6102c7565b604051901515815260200161010f565b6003545b60405190815260200161010f565b61012b61015b36600461094b565b6102e1565b6040516012815260200161010f565b61012b61017d366004610921565b610305565b610195610190366004610921565b610327565b005b61013f6101a5366004610987565b6001600160a01b031660009081526001602052604090205490565b6101956103ed565b6000546040516001600160a01b03909116815260200161010f565b610102610401565b61012b6101f9366004610921565b610410565b61012b61020c366004610921565b61048b565b61013f61021f3660046109a9565b610499565b610195610232366004610987565b50565b606060048054610244906109dc565b80601f0160208091040260200160405190810160405280929190818152602001828054610270906109dc565b80156102bd5780601f10610292576101008083540402835291602001916102bd565b820191906000526020600020905b8154815290600101906020018083116102a057829003601f168201915b5050505050905090565b6000336102d58185856104c4565b60019150505b92915050565b6000336102ef8582856105e8565b6102fa858585610662565b506001949350505050565b6000336102d58185856103188383610499565b6103229190610a16565b6104c4565b6001600160a01b0382166103825760405162461bcd60e51b815260206004820152601f60248201527f45524332303a206d696e7420746f20746865207a65726f20616464726573730060448201526064015b60405180910390fd5b80600360008282546103949190610a16565b90915550506001600160a01b0382166000818152600160209081526040808320805486019055518481527fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef910160405180910390a35050565b6103f561080d565b6103ff6000610867565b565b606060058054610244906109dc565b6000338161041e8286610499565b90508381101561047e5760405162461bcd60e51b815260206004820152602560248201527f45524332303a2064656372656173656420616c6c6f77616e63652062656c6f77604482015264207a65726f60d81b6064820152608401610379565b6102fa82868684036104c4565b6000336102d5818585610662565b6001600160a01b03918216600090815260026020908152604080832093909416825291909152205490565b6001600160a01b0383166105265760405162461bcd60e51b8152602060048201526024808201527f45524332303a20617070726f76652066726f6d20746865207a65726f206164646044820152637265737360e01b6064820152608401610379565b6001600160a01b0382166105875760405162461bcd60e51b815260206004820152602260248201527f45524332303a20617070726f766520746f20746865207a65726f206164647265604482015261737360f01b6064820152608401610379565b6001600160a01b0383811660008181526002602090815260408083209487168084529482529182902085905590518481527f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925910160405180910390a3505050565b60006105f48484610499565b9050600019811461065c578181101561064f5760405162461bcd60e51b815260206004820152601d60248201527f45524332303a20696e73756666696369656e7420616c6c6f77616e63650000006044820152606401610379565b61065c84848484036104c4565b50505050565b6001600160a01b0383166106c65760405162461bcd60e51b815260206004820152602560248201527f45524332303a207472616e736665722066726f6d20746865207a65726f206164604482015264647265737360d81b6064820152608401610379565b6001600160a01b0382166107285760405162461bcd60e51b815260206004820152602360248201527f45524332303a207472616e7366657220746f20746865207a65726f206164647260448201526265737360e81b6064820152608401610379565b6001600160a01b038316600090815260016020526040902054818110156107a05760405162461bcd60e51b815260206004820152602660248201527f45524332303a207472616e7366657220616d6f756e7420657863656564732062604482015265616c616e636560d01b6064820152608401610379565b6001600160a01b0380851660008181526001602052604080822086860390559286168082529083902080548601905591517fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef906108009086815260200190565b60405180910390a361065c565b6000546001600160a01b031633146103ff5760405162461bcd60e51b815260206004820181905260248201527f4f776e61626c653a2063616c6c6572206973206e6f7420746865206f776e65726044820152606401610379565b600080546001600160a01b038381166001600160a01b0319831681178455604051919092169283917f8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e09190a35050565b600060208083528351808285015260005b818110156108e4578581018301518582016040015282016108c8565b506000604082860101526040601f19601f8301168501019250505092915050565b80356001600160a01b038116811461091c57600080fd5b919050565b6000806040838503121561093457600080fd5b61093d83610905565b946020939093013593505050565b60008060006060848603121561096057600080fd5b61096984610905565b925061097760208501610905565b9150604084013590509250925092565b60006020828403121561099957600080fd5b6109a282610905565b9392505050565b600080604083850312156109bc57600080fd5b6109c583610905565b91506109d360208401610905565b90509250929050565b600181811c908216806109f057607f821691505b602082108103610a1057634e487b7160e01b600052602260045260246000fd5b50919050565b808201808211156102db57634e487b7160e01b600052601160045260246000fdfea264697066735822122049c8ef35009c5861516729fdf1dce5c8673c1603b1688c403f620e037f7b33a964736f6c63430008100033";

pub const TEST_KEYSTORE_DATA: &str = r#"{"crypto":{"cipher":"aes-128-ctr","cipherparams":{"iv":"479a498cce1c80a0ac51dd183f9b96ef"},"ciphertext":"ce40d0bab943352214fbd7923310ba607489e76bf2622ffebea558cbfb27c77b","kdf":"scrypt","kdfparams":{"dklen":32,"n":8192,"p":1,"r":8,"salt":"395438174df838577ec499d71ad5fe67ced8cf09e8096930619a6f427abf5cf8"},"mac":"5a41fed1220c6b78068f5f15d1009f5914abacf4eb9c36cb9aebc258f50b233b"},"id":"6726da52-1ae5-4f65-9469-04c2da8a35d6","version":3}"#;

pub fn generate_test_keystore_file() -> Result<String, Box<dyn std::error::Error>> {
    let keystore_file_path = std::env::temp_dir().join(TEST_KEYSTORE_FILE_NAME);
    let mut keystore_file = std::fs::File::create(keystore_file_path.clone())?;
    writeln!(&mut keystore_file, "{}", TEST_KEYSTORE_DATA)?;
    Ok(keystore_file_path
        .as_os_str()
        .to_str()
        .expect("valid keystore file path")
        .to_string())
}
