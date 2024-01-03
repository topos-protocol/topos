use ethers::{
    contract::abigen,
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    prelude::Wallet,
    providers::{Http, Provider},
};

//TODO I haven't find a way to parametrize version, macro accepts strictly string literal
// `topos-smart-contracts` build artifacts directory must be copied to the root topos directory to run these tests
abigen!(
    TokenDeployerContract,
    "./artifacts/contracts/topos-core/TokenDeployer.sol/TokenDeployer.json"
);
abigen!(
    ToposCoreContract,
    "./artifacts/contracts/topos-core/ToposCore.sol/ToposCore.json"
);
abigen!(
    ToposCoreProxyContract,
    "./artifacts/contracts/topos-core/ToposCoreProxy.sol/ToposCoreProxy.json"
);
abigen!(
    ToposMessagingContract,
    "./artifacts/contracts/topos-core/ToposMessaging.sol/ToposMessaging.json"
);
abigen!(
    ERC20MessagingContract,
    "./artifacts/contracts/examples/ERC20Messaging.sol/ERC20Messaging.json"
);
abigen!(
    IToposCore,
    "./artifacts/contracts/interfaces/IToposCore.sol/IToposCore.json"
);
abigen!(
    IToposMessaging,
    "./artifacts/contracts/interfaces/IToposMessaging.sol/IToposMessaging.json"
);
abigen!(
    IERC20Messaging,
    "./artifacts/contracts/interfaces/IERC20Messaging.sol/IERC20Messaging.json"
);

abigen!(
    IERC20,
    r"[
       function totalSupply() external view returns (uint)

       function balanceOf(address account) external view returns (uint)

       function transfer(address recipient, uint amount) external returns (bool)

       function allowance(address owner, address spender) external view returns (uint)

       function approve(address spender, uint amount) external returns (bool)

       function transferFrom(address sender, address recipient, uint amount) external returns (bool)
       ]"
);

pub type IToposCoreClient = IToposCore<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
pub type IToposMessagingClient =
    IToposMessaging<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
pub type IERC20Client = IERC20<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
pub type IERC20MessagingClient =
    IERC20Messaging<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
