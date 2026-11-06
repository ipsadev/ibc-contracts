// Code generated - DO NOT EDIT.
// This file is a generated binding and any manual changes will be lost.

package ibcerc20

import (
	"context"
	"errors"
	"math/big"
	"strings"
	"time"

	ethereum "github.com/ethereum/go-ethereum"
	"github.com/ethereum/go-ethereum/accounts/abi"
	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/event"
)

// Reference imports to suppress errors if they are not otherwise used.
var (
	_ = errors.New
	_ = big.NewInt
	_ = strings.NewReader
	_ = ethereum.NotFound
	_ = bind.Bind
	_ = common.Big1
	_ = types.BloomLookup
	_ = event.NewSubscription
	_ = abi.ConvertType
	_ = time.Tick
	_ = context.Background
)

// ContractMetaData contains all meta data concerning the Contract contract.
var ContractMetaData = &bind.MetaData{
	ABI: "[{\"type\":\"constructor\",\"inputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"allowance\",\"inputs\":[{\"name\":\"owner\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"spender\",\"type\":\"address\",\"internalType\":\"address\"}],\"outputs\":[{\"name\":\"\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"approve\",\"inputs\":[{\"name\":\"spender\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"outputs\":[{\"name\":\"\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"balanceOf\",\"inputs\":[{\"name\":\"account\",\"type\":\"address\",\"internalType\":\"address\"}],\"outputs\":[{\"name\":\"\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"burn\",\"inputs\":[{\"name\":\"mintAddress\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"amount\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"decimals\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"uint8\",\"internalType\":\"uint8\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"escrow\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"address\",\"internalType\":\"address\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"fullDenomPath\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"string\",\"internalType\":\"string\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"ics20\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"address\",\"internalType\":\"address\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"initialize\",\"inputs\":[{\"name\":\"ics20_\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"escrow_\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"fullDenomPath_\",\"type\":\"string\",\"internalType\":\"string\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"mint\",\"inputs\":[{\"name\":\"mintAddress\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"amount\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"name\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"string\",\"internalType\":\"string\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"setMetadata\",\"inputs\":[{\"name\":\"customDecimals\",\"type\":\"uint8\",\"internalType\":\"uint8\"},{\"name\":\"customName\",\"type\":\"string\",\"internalType\":\"string\"},{\"name\":\"customSymbol\",\"type\":\"string\",\"internalType\":\"string\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"symbol\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"string\",\"internalType\":\"string\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"totalSupply\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"transfer\",\"inputs\":[{\"name\":\"to\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"outputs\":[{\"name\":\"\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"transferFrom\",\"inputs\":[{\"name\":\"from\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"to\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"outputs\":[{\"name\":\"\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"event\",\"name\":\"Approval\",\"inputs\":[{\"name\":\"owner\",\"type\":\"address\",\"indexed\":true,\"internalType\":\"address\"},{\"name\":\"spender\",\"type\":\"address\",\"indexed\":true,\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"indexed\":false,\"internalType\":\"uint256\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"Initialized\",\"inputs\":[{\"name\":\"version\",\"type\":\"uint64\",\"indexed\":false,\"internalType\":\"uint64\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"Transfer\",\"inputs\":[{\"name\":\"from\",\"type\":\"address\",\"indexed\":true,\"internalType\":\"address\"},{\"name\":\"to\",\"type\":\"address\",\"indexed\":true,\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"indexed\":false,\"internalType\":\"uint256\"}],\"anonymous\":false},{\"type\":\"error\",\"name\":\"ERC20InsufficientAllowance\",\"inputs\":[{\"name\":\"spender\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"allowance\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"needed\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"type\":\"error\",\"name\":\"ERC20InsufficientBalance\",\"inputs\":[{\"name\":\"sender\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"balance\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"needed\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"type\":\"error\",\"name\":\"ERC20InvalidApprover\",\"inputs\":[{\"name\":\"approver\",\"type\":\"address\",\"internalType\":\"address\"}]},{\"type\":\"error\",\"name\":\"ERC20InvalidReceiver\",\"inputs\":[{\"name\":\"receiver\",\"type\":\"address\",\"internalType\":\"address\"}]},{\"type\":\"error\",\"name\":\"ERC20InvalidSender\",\"inputs\":[{\"name\":\"sender\",\"type\":\"address\",\"internalType\":\"address\"}]},{\"type\":\"error\",\"name\":\"ERC20InvalidSpender\",\"inputs\":[{\"name\":\"spender\",\"type\":\"address\",\"internalType\":\"address\"}]},{\"type\":\"error\",\"name\":\"IBCERC20DecimalsAlreadySet\",\"inputs\":[{\"name\":\"decimals\",\"type\":\"uint8\",\"internalType\":\"uint8\"},{\"name\":\"requestedDecimals\",\"type\":\"uint8\",\"internalType\":\"uint8\"}]},{\"type\":\"error\",\"name\":\"IBCERC20NotEscrow\",\"inputs\":[{\"name\":\"escrow\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"mintAddress\",\"type\":\"address\",\"internalType\":\"address\"}]},{\"type\":\"error\",\"name\":\"IBCERC20Unauthorized\",\"inputs\":[{\"name\":\"caller\",\"type\":\"address\",\"internalType\":\"address\"}]},{\"type\":\"error\",\"name\":\"InvalidInitialization\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotInitializing\",\"inputs\":[]}]",
	Bin: "0x6080806040523460aa575f5160206123025f395f51905f525460ff8160401c16609b576002600160401b03196001600160401b038216016049575b60405161225390816100af8239f35b6001600160401b0319166001600160401b039081175f5160206123025f395f51905f525581527fc7f505b2f371ae2175ee4913f4499e1f2633a7b5936321eed1cdaeb6115181d290602090a15f80603a565b63f92ee8a960e01b5f5260045ffd5b5f80fdfe60806040526004361015610011575f80fd5b5f3560e01c806306fdde0314611ced578063095ea7b314611c3d57806318160ddd14611c0157806323b872dd14611a36578063313ce567146119d857806340c10f19146118105780634571e3a614610f3f57806370a0823114610edb57806395d89b4114610c6b5780639a61e8e114610c195780639dc29fac14610a24578063a9059cbb146109f3578063b090e66c146102e8578063dd62ed3e14610255578063e2fdcc17146102035763f440e8c4146100c9575f80fd5b346101ff575f6003193601126101ff576040515f7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8005461010881611f70565b80845290600181169081156101bd5750600114610140575b61013c8361013081850382611fc1565b60405191829182611ed2565b0390f35b7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8005f9081527f5582d9e466460b8f89b1de7e4e29794e195d6a38d28bf83a62613d236c220fa7939250905b8082106101a357509091508101602001610130610120565b91926001816020925483858801015201910190929161018b565b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff001660208086019190915291151560051b840190910191506101309050610120565b5f80fd5b346101ff575f6003193601126101ff57602073ffffffffffffffffffffffffffffffffffffffff7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8015416604051908152f35b346101ff5760406003193601126101ff5761026e611efc565b73ffffffffffffffffffffffffffffffffffffffff6102d261028e611f1f565b9273ffffffffffffffffffffffffffffffffffffffff165f527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0160205260405f2090565b91165f52602052602060405f2054604051908152f35b346101ff5760606003193601126101ff5760043560ff8116908181036101ff5760243567ffffffffffffffff81116101ff57610328903690600401611f42565b60449391933567ffffffffffffffff81116101ff5761034b903690600401611f42565b9290937f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80254916040517fbf7e214f00000000000000000000000000000000000000000000000000000000815260208160048173ffffffffffffffffffffffffffffffffffffffff88165afa908115610998575f916109a3575b50604073ffffffffffffffffffffffffffffffffffffffff9160448251809481937fd1f856ee00000000000000000000000000000000000000000000000000000000835260076004840152336024840152165afa8015610998575f905f90610946575b61043a925081610937575b503390611fe4565b7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea805549060ff8216158015610928575b156108f3575075ff00000000000000000000000000000000000000000074010000000000000000000000000000000000000000939260017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff007fffffffffffffffffffff0000ffffffffffffffffffffffffffffffffffffffff9416177f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8055560a81b16911617177f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8025567ffffffffffffffff811161078c5761056a7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80354611f70565b601f8111610866575b505f601f82116001146107c45781906105a29394955f926107b9575b50505f198260011b9260031b1c19161790565b7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea803555b67ffffffffffffffff811161078c576105fe7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80454611f70565b601f81116106ff575b505f601f8211600114610662578190610633935f926106575750505f198260011b9260031b1c19161790565b7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80455005b01359050838061058f565b601f198216927f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8045f5260205f20915f5b8581106106e7575083600195106106ce575b505050811b017f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80455005b5f1960f88560031b161c199101351690558280806106a4565b90926020600181928686013581550194019101610692565b81811115610607577f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8045f52601f820160051c7ff51bc0aa5945c62aed32a8905c4aab48c5e363030c52f60cc949a44bc6c4c3ae60208410610784575b81601f9101920160051c03905f5b828110610777575050610607565b5f82820155600101610769565b5f915061075b565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b01359050858061058f565b601f198216947f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8035f5260205f20915f5b87811061084e575083600195969710610835575b505050811b017f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea803556105c5565b5f1960f88560031b161c19910135169055848080610808565b909260206001819286860135815501940191016107f4565b81811115610573577f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8035f52601f820160051c7f45a9083052d46c6d582a6ff858a1112135ef869ec6b0f630ea93985a51ff3d12602084106108eb575b81601f9101920160051c03905f5b8281106108de575050610573565b5f828201556001016108d0565b5f91506108c2565b60ff847f0d9020c8000000000000000000000000000000000000000000000000000000005f5260a81c1660045260245260445ffd5b5060ff8460a81c168114610469565b63ffffffff9150161589610432565b50506040813d604011610990575b8161096160409383611fc1565b810103126101ff5780519081151582036101ff576020015163ffffffff811681036101ff5761043a9190610427565b3d9150610954565b6040513d5f823e3d90fd5b90506020813d6020116109eb575b816109be60209383611fc1565b810103126101ff575173ffffffffffffffffffffffffffffffffffffffff811681036101ff5760406103c4565b3d91506109b1565b346101ff5760406003193601126101ff57610a19610a0f611efc565b60243590336120c5565b602060405160018152f35b346101ff5760406003193601126101ff57610a3d611efc565b60243590610a843373ffffffffffffffffffffffffffffffffffffffff7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80254163314611fe4565b610ae073ffffffffffffffffffffffffffffffffffffffff7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80154169173ffffffffffffffffffffffffffffffffffffffff81169280841461202e565b8015610bed57805f527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0060205260405f2054828110610bbb576020835f947fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef938587527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace008452036040862055807f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0254037f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0255604051908152a3005b907fe450d38c000000000000000000000000000000000000000000000000000000005f5260045260245260445260645ffd5b7f96c6fd1e000000000000000000000000000000000000000000000000000000005f525f60045260245ffd5b346101ff575f6003193601126101ff57602073ffffffffffffffffffffffffffffffffffffffff7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8025416604051908152f35b346101ff575f6003193601126101ff577f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8025460a01c60ff1615610dbf576040515f7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80454610cd781611f70565b8084529060018116908115610d7d5750600114610d00575b50906101308161013c930382611fc1565b7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8045f9081527ff51bc0aa5945c62aed32a8905c4aab48c5e363030c52f60cc949a44bc6c4c3ae939250905b808210610d6357509091508101602001610130610cef565b919260018160209254838588010152019101909291610d4b565b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff001660208086019190915291151560051b840190910191506101309050610cef565b6040515f7f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0454610dee81611f70565b8084529060018116908115610e995750600114610e1c575b5090610e178161013c930382611fc1565b610130565b7f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace045f9081527f46a2803e59a4de4e7a4c574b1243f25977ac4c77d5a1a4a609b5394cebb4a2aa939250905b808210610e7f57509091508101602001610e17610e06565b919260018160209254838588010152019101909291610e67565b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff001660208086019190915291151560051b84019091019150610e179050610e06565b346101ff5760206003193601126101ff5773ffffffffffffffffffffffffffffffffffffffff610f09611efc565b165f527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00602052602060405f2054604051908152f35b346101ff5760606003193601126101ff57610f58611efc565b610f60611f1f565b9060443567ffffffffffffffff81116101ff57610f81903690600401611f42565b9190927ff0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00549260ff8460401c16159367ffffffffffffffff811680159081611808575b60011490816117fe575b1590816117f5575b506117cd578460017fffffffffffffffffffffffffffffffffffffffffffffffff00000000000000008316177ff0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a0055611778575b5061103536828761207f565b61104036838861207f565b906110496121ef565b6110516121ef565b80519067ffffffffffffffff821161078c5761108d7f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0354611f70565b601f81116116eb575b50602090601f8311600114611648576110c492915f918361163d5750505f198260011b9260031b1c19161790565b7f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace03555b80519067ffffffffffffffff821161078c576111237f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0454611f70565b601f81116115b0575b50602090601f831160011461150d5761115a92915f91836115025750505f198260011b9260031b1c19161790565b7f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace04555b67ffffffffffffffff811161078c576111b67f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80054611f70565b601f8111611475575b505f601f821160011461139c578173ffffffffffffffffffffffffffffffffffffffff959692611205928795945f926113915750505f198260011b9260031b1c19161790565b7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea800555b167fffffffffffffffffffffffff00000000000000000000000000000000000000007f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8015416177f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80155167fffffffffffffffffffffffff00000000000000000000000000000000000000007f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8025416177f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea802556112fe57005b7fffffffffffffffffffffffffffffffffffffffffffffff00ffffffffffffffff7ff0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a0054167ff0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00557fc7f505b2f371ae2175ee4913f4499e1f2633a7b5936321eed1cdaeb6115181d2602060405160018152a1005b01359050888061058f565b601f198216907f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8005f527f5582d9e466460b8f89b1de7e4e29794e195d6a38d28bf83a62613d236c220fa7915f5b81811061145d57509173ffffffffffffffffffffffffffffffffffffffff9697849288969560019510611444575b505050811b017f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80055611228565b5f1960f88560031b161c19910135169055878080611417565b9192602060018192868c0135815501940192016113e9565b818111156111bf577f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8005f52601f820160051c7f5582d9e466460b8f89b1de7e4e29794e195d6a38d28bf83a62613d236c220fa7602084106114fa575b81601f9101920160051c03905f5b8281106114ed5750506111bf565b5f828201556001016114df565b5f91506114d1565b01519050888061058f565b90601f198316917f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace045f52815f20925f5b8181106115985750908460019594939210611580575b505050811b017f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace045561117d565b01515f1960f88460031b161c19169055878080611553565b9293602060018192878601518155019501930161153d565b8281111561112c577f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace045f52601f830160051c7f46a2803e59a4de4e7a4c574b1243f25977ac4c77d5a1a4a609b5394cebb4a2aa60208510611635575b81601f9101920160051c03905f5b82811061162857505061112c565b5f8282015560010161161a565b5f915061160c565b01519050898061058f565b90601f198316917f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace035f52815f20925f5b8181106116d357509084600195949392106116bb575b505050811b017f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace03556110e7565b01515f1960f88460031b161c1916905588808061168e565b92936020600181928786015181550195019301611678565b82811115611096577f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace035f52601f830160051c7f2ae08a8e29253f69ac5d979a101956ab8f8d9d7ded63fa7a83b16fc47648eab060208510611770575b81601f9101920160051c03905f5b828110611763575050611096565b5f82820155600101611755565b5f9150611747565b7fffffffffffffffffffffffffffffffffffffffffffffff0000000000000000001668010000000000000001177ff0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a005585611029565b7ff92ee8a9000000000000000000000000000000000000000000000000000000005f5260045ffd5b90501587610fd6565b303b159150610fce565b869150610fc4565b346101ff5760406003193601126101ff57611829611efc565b60243561186f3373ffffffffffffffffffffffffffffffffffffffff7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80254163314611fe4565b6118cb73ffffffffffffffffffffffffffffffffffffffff7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80154169273ffffffffffffffffffffffffffffffffffffffff81169380851461202e565b81156119ac577f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace02549080820180921161197f5760207fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef915f937f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace02558484527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00825260408420818154019055604051908152a3005b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b7fec442f05000000000000000000000000000000000000000000000000000000005f525f60045260245ffd5b346101ff575f6003193601126101ff577f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8025460ff8160a01c165f14611a2c5760ff60209160a81c165b60ff60405191168152f35b5060206012611a21565b346101ff5760606003193601126101ff57611a4f611efc565b611a57611f1f565b60443590611aa28373ffffffffffffffffffffffffffffffffffffffff165f527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0160205260405f2090565b73ffffffffffffffffffffffffffffffffffffffff33165f5260205260405f2054925f198410611ad7575b610a1993506120c5565b828410611bcd5773ffffffffffffffffffffffffffffffffffffffff811615611ba1573315611b7557610a1993611b4b8273ffffffffffffffffffffffffffffffffffffffff165f527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0160205260405f2090565b73ffffffffffffffffffffffffffffffffffffffff33165f526020528360405f2091039055611acd565b7f94280d62000000000000000000000000000000000000000000000000000000005f525f60045260245ffd5b7fe602df05000000000000000000000000000000000000000000000000000000005f525f60045260245ffd5b82847ffb8f41b2000000000000000000000000000000000000000000000000000000005f523360045260245260445260645ffd5b346101ff575f6003193601126101ff5760207f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0254604051908152f35b346101ff5760406003193601126101ff57611c56611efc565b602435903315611ba15773ffffffffffffffffffffffffffffffffffffffff16908115611b7557335f9081527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0160205260409020825f526020528060405f20556040519081527f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b92560203392a3602060405160018152f35b346101ff575f6003193601126101ff577f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8025460a01c60ff1615611dfe576040515f7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea80354611d5981611f70565b8084529060018116908115610d7d5750600114611d815750906101308161013c930382611fc1565b7f1dd677b5a02f77610493322b5fdbbfdb607b541c6e6045daab3464e895dea8035f9081527f45a9083052d46c6d582a6ff858a1112135ef869ec6b0f630ea93985a51ff3d12939250905b808210611de457509091508101602001610130610cef565b919260018160209254838588010152019101909291611dcc565b6040515f7f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0354611e2d81611f70565b8084529060018116908115610e995750600114611e55575090610e178161013c930382611fc1565b7f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace035f9081527f2ae08a8e29253f69ac5d979a101956ab8f8d9d7ded63fa7a83b16fc47648eab0939250905b808210611eb857509091508101602001610e17610e06565b919260018160209254838588010152019101909291611ea0565b601f19601f602060409481855280519182918282880152018686015e5f8582860101520116010190565b6004359073ffffffffffffffffffffffffffffffffffffffff821682036101ff57565b6024359073ffffffffffffffffffffffffffffffffffffffff821682036101ff57565b9181601f840112156101ff5782359167ffffffffffffffff83116101ff57602083818601950101116101ff57565b90600182811c92168015611fb7575b6020831014611f8a57565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602260045260245ffd5b91607f1691611f7f565b90601f601f19910116810190811067ffffffffffffffff82111761078c57604052565b15611fec5750565b73ffffffffffffffffffffffffffffffffffffffff907f67f9ac67000000000000000000000000000000000000000000000000000000005f521660045260245ffd5b15612037575050565b9073ffffffffffffffffffffffffffffffffffffffff80927fe3985731000000000000000000000000000000000000000000000000000000005f52166004521660245260445ffd5b92919267ffffffffffffffff821161078c57604051916120a96020601f19601f8401160184611fc1565b8294818452818301116101ff578281602093845f960137010152565b73ffffffffffffffffffffffffffffffffffffffff16908115610bed5773ffffffffffffffffffffffffffffffffffffffff169182156119ac57815f527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0060205260405f20548181106121bd57817fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef92602092855f527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace0084520360405f2055845f527f52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00825260405f20818154019055604051908152a3565b827fe450d38c000000000000000000000000000000000000000000000000000000005f5260045260245260445260645ffd5b60ff7ff0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a005460401c161561221e57565b7fd7e6bcf8000000000000000000000000000000000000000000000000000000005f5260045ffdfea164736f6c6343000825000af0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00",
}

// ContractABI is the input ABI used to generate the binding from.
// Deprecated: Use ContractMetaData.ABI instead.
var ContractABI = ContractMetaData.ABI

// ContractBin is the compiled bytecode used for deploying new contracts.
// Deprecated: Use ContractMetaData.Bin instead.
var ContractBin = ContractMetaData.Bin

// DeployContract deploys a new Ethereum contract, binding an instance of Contract to it.
func DeployContract(auth *bind.TransactOpts, backend bind.ContractBackend) (common.Address, *types.Transaction, *Contract, error) {
	parsed, err := ContractMetaData.GetAbi()
	if err != nil {
		return common.Address{}, nil, nil, err
	}
	if parsed == nil {
		return common.Address{}, nil, nil, errors.New("GetABI returned nil")
	}

	address, tx, contract, err := bind.DeployContract(auth, *parsed, common.FromHex(ContractBin), backend)
	if err != nil {
		return common.Address{}, nil, nil, err
	}
	return address, tx, &Contract{ContractCaller: ContractCaller{contract: contract}, ContractTransactor: ContractTransactor{contract: contract}, ContractFilterer: ContractFilterer{contract: contract}}, nil
}

// Contract is an auto generated Go binding around an Ethereum contract.
type Contract struct {
	ContractCaller     // Read-only binding to the contract
	ContractTransactor // Write-only binding to the contract
	ContractFilterer   // Log filterer for contract events
}

// ContractCaller is an auto generated read-only Go binding around an Ethereum contract.
type ContractCaller struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// ContractTransactor is an auto generated write-only Go binding around an Ethereum contract.
type ContractTransactor struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// ContractFilterer is an auto generated log filtering Go binding around an Ethereum contract events.
type ContractFilterer struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// ContractSession is an auto generated Go binding around an Ethereum contract,
// with pre-set call and transact options.
type ContractSession struct {
	Contract     *Contract         // Generic contract binding to set the session for
	CallOpts     bind.CallOpts     // Call options to use throughout this session
	TransactOpts bind.TransactOpts // Transaction auth options to use throughout this session
}

// ContractCallerSession is an auto generated read-only Go binding around an Ethereum contract,
// with pre-set call options.
type ContractCallerSession struct {
	Contract *ContractCaller // Generic contract caller binding to set the session for
	CallOpts bind.CallOpts   // Call options to use throughout this session
}

// ContractTransactorSession is an auto generated write-only Go binding around an Ethereum contract,
// with pre-set transact options.
type ContractTransactorSession struct {
	Contract     *ContractTransactor // Generic contract transactor binding to set the session for
	TransactOpts bind.TransactOpts   // Transaction auth options to use throughout this session
}

// ContractRaw is an auto generated low-level Go binding around an Ethereum contract.
type ContractRaw struct {
	Contract *Contract // Generic contract binding to access the raw methods on
}

// ContractCallerRaw is an auto generated low-level read-only Go binding around an Ethereum contract.
type ContractCallerRaw struct {
	Contract *ContractCaller // Generic read-only contract binding to access the raw methods on
}

// ContractTransactorRaw is an auto generated low-level write-only Go binding around an Ethereum contract.
type ContractTransactorRaw struct {
	Contract *ContractTransactor // Generic write-only contract binding to access the raw methods on
}

// NewContract creates a new instance of Contract, bound to a specific deployed contract.
func NewContract(address common.Address, backend bind.ContractBackend) (*Contract, error) {
	contract, err := bindContract(address, backend, backend, backend)
	if err != nil {
		return nil, err
	}
	return &Contract{ContractCaller: ContractCaller{contract: contract}, ContractTransactor: ContractTransactor{contract: contract}, ContractFilterer: ContractFilterer{contract: contract}}, nil
}

// NewContractCaller creates a new read-only instance of Contract, bound to a specific deployed contract.
func NewContractCaller(address common.Address, caller bind.ContractCaller) (*ContractCaller, error) {
	contract, err := bindContract(address, caller, nil, nil)
	if err != nil {
		return nil, err
	}
	return &ContractCaller{contract: contract}, nil
}

// NewContractTransactor creates a new write-only instance of Contract, bound to a specific deployed contract.
func NewContractTransactor(address common.Address, transactor bind.ContractTransactor) (*ContractTransactor, error) {
	contract, err := bindContract(address, nil, transactor, nil)
	if err != nil {
		return nil, err
	}
	return &ContractTransactor{contract: contract}, nil
}

// NewContractFilterer creates a new log filterer instance of Contract, bound to a specific deployed contract.
func NewContractFilterer(address common.Address, filterer bind.ContractFilterer) (*ContractFilterer, error) {
	contract, err := bindContract(address, nil, nil, filterer)
	if err != nil {
		return nil, err
	}
	return &ContractFilterer{contract: contract}, nil
}

// bindContract binds a generic wrapper to an already deployed contract.
func bindContract(address common.Address, caller bind.ContractCaller, transactor bind.ContractTransactor, filterer bind.ContractFilterer) (*bind.BoundContract, error) {
	parsed, err := ContractMetaData.GetAbi()
	if err != nil {
		return nil, err
	}
	return bind.NewBoundContract(address, *parsed, caller, transactor, filterer), nil
}

// Call invokes the (constant) contract method with params as input values and
// sets the output to result. The result type might be a single field for simple
// returns, a slice of interfaces for anonymous returns and a struct for named
// returns.
func (_Contract *ContractRaw) Call(opts *bind.CallOpts, result *[]interface{}, method string, params ...interface{}) error {
	return _Contract.Contract.ContractCaller.contract.Call(opts, result, method, params...)
}

// Transfer initiates a plain transaction to move funds to the contract, calling
// its default method if one is available.
func (_Contract *ContractRaw) Transfer(opts *bind.TransactOpts) (*types.Transaction, error) {
	return _Contract.Contract.ContractTransactor.contract.Transfer(opts)
}

// Transact invokes the (paid) contract method with params as input values.
func (_Contract *ContractRaw) Transact(opts *bind.TransactOpts, method string, params ...interface{}) (*types.Transaction, error) {
	return _Contract.Contract.ContractTransactor.contract.Transact(opts, method, params...)
}

// Call invokes the (constant) contract method with params as input values and
// sets the output to result. The result type might be a single field for simple
// returns, a slice of interfaces for anonymous returns and a struct for named
// returns.
func (_Contract *ContractCallerRaw) Call(opts *bind.CallOpts, result *[]interface{}, method string, params ...interface{}) error {
	return _Contract.Contract.contract.Call(opts, result, method, params...)
}

// Transfer initiates a plain transaction to move funds to the contract, calling
// its default method if one is available.
func (_Contract *ContractTransactorRaw) Transfer(opts *bind.TransactOpts) (*types.Transaction, error) {
	return _Contract.Contract.contract.Transfer(opts)
}

// Transact invokes the (paid) contract method with params as input values.
func (_Contract *ContractTransactorRaw) Transact(opts *bind.TransactOpts, method string, params ...interface{}) (*types.Transaction, error) {
	return _Contract.Contract.contract.Transact(opts, method, params...)
}

// Allowance is a free data retrieval call binding the contract method 0xdd62ed3e.
//
// Solidity: function allowance(address owner, address spender) view returns(uint256)
func (_Contract *ContractCaller) Allowance(opts *bind.CallOpts, owner common.Address, spender common.Address) (*big.Int, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "allowance", owner, spender)

	if err != nil {
		return *new(*big.Int), err
	}

	out0 := *abi.ConvertType(out[0], new(*big.Int)).(**big.Int)

	return out0, err

}

// Allowance is a free data retrieval call binding the contract method 0xdd62ed3e.
//
// Solidity: function allowance(address owner, address spender) view returns(uint256)
func (_Contract *ContractSession) Allowance(owner common.Address, spender common.Address) (*big.Int, error) {
	return _Contract.Contract.Allowance(&_Contract.CallOpts, owner, spender)
}

// Allowance is a free data retrieval call binding the contract method 0xdd62ed3e.
//
// Solidity: function allowance(address owner, address spender) view returns(uint256)
func (_Contract *ContractCallerSession) Allowance(owner common.Address, spender common.Address) (*big.Int, error) {
	return _Contract.Contract.Allowance(&_Contract.CallOpts, owner, spender)
}

// BalanceOf is a free data retrieval call binding the contract method 0x70a08231.
//
// Solidity: function balanceOf(address account) view returns(uint256)
func (_Contract *ContractCaller) BalanceOf(opts *bind.CallOpts, account common.Address) (*big.Int, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "balanceOf", account)

	if err != nil {
		return *new(*big.Int), err
	}

	out0 := *abi.ConvertType(out[0], new(*big.Int)).(**big.Int)

	return out0, err

}

// BalanceOf is a free data retrieval call binding the contract method 0x70a08231.
//
// Solidity: function balanceOf(address account) view returns(uint256)
func (_Contract *ContractSession) BalanceOf(account common.Address) (*big.Int, error) {
	return _Contract.Contract.BalanceOf(&_Contract.CallOpts, account)
}

// BalanceOf is a free data retrieval call binding the contract method 0x70a08231.
//
// Solidity: function balanceOf(address account) view returns(uint256)
func (_Contract *ContractCallerSession) BalanceOf(account common.Address) (*big.Int, error) {
	return _Contract.Contract.BalanceOf(&_Contract.CallOpts, account)
}

// Decimals is a free data retrieval call binding the contract method 0x313ce567.
//
// Solidity: function decimals() view returns(uint8)
func (_Contract *ContractCaller) Decimals(opts *bind.CallOpts) (uint8, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "decimals")

	if err != nil {
		return *new(uint8), err
	}

	out0 := *abi.ConvertType(out[0], new(uint8)).(*uint8)

	return out0, err

}

// Decimals is a free data retrieval call binding the contract method 0x313ce567.
//
// Solidity: function decimals() view returns(uint8)
func (_Contract *ContractSession) Decimals() (uint8, error) {
	return _Contract.Contract.Decimals(&_Contract.CallOpts)
}

// Decimals is a free data retrieval call binding the contract method 0x313ce567.
//
// Solidity: function decimals() view returns(uint8)
func (_Contract *ContractCallerSession) Decimals() (uint8, error) {
	return _Contract.Contract.Decimals(&_Contract.CallOpts)
}

// Escrow is a free data retrieval call binding the contract method 0xe2fdcc17.
//
// Solidity: function escrow() view returns(address)
func (_Contract *ContractCaller) Escrow(opts *bind.CallOpts) (common.Address, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "escrow")

	if err != nil {
		return *new(common.Address), err
	}

	out0 := *abi.ConvertType(out[0], new(common.Address)).(*common.Address)

	return out0, err

}

// Escrow is a free data retrieval call binding the contract method 0xe2fdcc17.
//
// Solidity: function escrow() view returns(address)
func (_Contract *ContractSession) Escrow() (common.Address, error) {
	return _Contract.Contract.Escrow(&_Contract.CallOpts)
}

// Escrow is a free data retrieval call binding the contract method 0xe2fdcc17.
//
// Solidity: function escrow() view returns(address)
func (_Contract *ContractCallerSession) Escrow() (common.Address, error) {
	return _Contract.Contract.Escrow(&_Contract.CallOpts)
}

// FullDenomPath is a free data retrieval call binding the contract method 0xf440e8c4.
//
// Solidity: function fullDenomPath() view returns(string)
func (_Contract *ContractCaller) FullDenomPath(opts *bind.CallOpts) (string, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "fullDenomPath")

	if err != nil {
		return *new(string), err
	}

	out0 := *abi.ConvertType(out[0], new(string)).(*string)

	return out0, err

}

// FullDenomPath is a free data retrieval call binding the contract method 0xf440e8c4.
//
// Solidity: function fullDenomPath() view returns(string)
func (_Contract *ContractSession) FullDenomPath() (string, error) {
	return _Contract.Contract.FullDenomPath(&_Contract.CallOpts)
}

// FullDenomPath is a free data retrieval call binding the contract method 0xf440e8c4.
//
// Solidity: function fullDenomPath() view returns(string)
func (_Contract *ContractCallerSession) FullDenomPath() (string, error) {
	return _Contract.Contract.FullDenomPath(&_Contract.CallOpts)
}

// Ics20 is a free data retrieval call binding the contract method 0x9a61e8e1.
//
// Solidity: function ics20() view returns(address)
func (_Contract *ContractCaller) Ics20(opts *bind.CallOpts) (common.Address, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "ics20")

	if err != nil {
		return *new(common.Address), err
	}

	out0 := *abi.ConvertType(out[0], new(common.Address)).(*common.Address)

	return out0, err

}

// Ics20 is a free data retrieval call binding the contract method 0x9a61e8e1.
//
// Solidity: function ics20() view returns(address)
func (_Contract *ContractSession) Ics20() (common.Address, error) {
	return _Contract.Contract.Ics20(&_Contract.CallOpts)
}

// Ics20 is a free data retrieval call binding the contract method 0x9a61e8e1.
//
// Solidity: function ics20() view returns(address)
func (_Contract *ContractCallerSession) Ics20() (common.Address, error) {
	return _Contract.Contract.Ics20(&_Contract.CallOpts)
}

// Name is a free data retrieval call binding the contract method 0x06fdde03.
//
// Solidity: function name() view returns(string)
func (_Contract *ContractCaller) Name(opts *bind.CallOpts) (string, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "name")

	if err != nil {
		return *new(string), err
	}

	out0 := *abi.ConvertType(out[0], new(string)).(*string)

	return out0, err

}

// Name is a free data retrieval call binding the contract method 0x06fdde03.
//
// Solidity: function name() view returns(string)
func (_Contract *ContractSession) Name() (string, error) {
	return _Contract.Contract.Name(&_Contract.CallOpts)
}

// Name is a free data retrieval call binding the contract method 0x06fdde03.
//
// Solidity: function name() view returns(string)
func (_Contract *ContractCallerSession) Name() (string, error) {
	return _Contract.Contract.Name(&_Contract.CallOpts)
}

// Symbol is a free data retrieval call binding the contract method 0x95d89b41.
//
// Solidity: function symbol() view returns(string)
func (_Contract *ContractCaller) Symbol(opts *bind.CallOpts) (string, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "symbol")

	if err != nil {
		return *new(string), err
	}

	out0 := *abi.ConvertType(out[0], new(string)).(*string)

	return out0, err

}

// Symbol is a free data retrieval call binding the contract method 0x95d89b41.
//
// Solidity: function symbol() view returns(string)
func (_Contract *ContractSession) Symbol() (string, error) {
	return _Contract.Contract.Symbol(&_Contract.CallOpts)
}

// Symbol is a free data retrieval call binding the contract method 0x95d89b41.
//
// Solidity: function symbol() view returns(string)
func (_Contract *ContractCallerSession) Symbol() (string, error) {
	return _Contract.Contract.Symbol(&_Contract.CallOpts)
}

// TotalSupply is a free data retrieval call binding the contract method 0x18160ddd.
//
// Solidity: function totalSupply() view returns(uint256)
func (_Contract *ContractCaller) TotalSupply(opts *bind.CallOpts) (*big.Int, error) {
	var out []interface{}
	err := _Contract.contract.Call(opts, &out, "totalSupply")

	if err != nil {
		return *new(*big.Int), err
	}

	out0 := *abi.ConvertType(out[0], new(*big.Int)).(**big.Int)

	return out0, err

}

// TotalSupply is a free data retrieval call binding the contract method 0x18160ddd.
//
// Solidity: function totalSupply() view returns(uint256)
func (_Contract *ContractSession) TotalSupply() (*big.Int, error) {
	return _Contract.Contract.TotalSupply(&_Contract.CallOpts)
}

// TotalSupply is a free data retrieval call binding the contract method 0x18160ddd.
//
// Solidity: function totalSupply() view returns(uint256)
func (_Contract *ContractCallerSession) TotalSupply() (*big.Int, error) {
	return _Contract.Contract.TotalSupply(&_Contract.CallOpts)
}

// Approve is a paid mutator transaction binding the contract method 0x095ea7b3.
//
// Solidity: function approve(address spender, uint256 value) returns(bool)
func (_Contract *ContractTransactor) Approve(opts *bind.TransactOpts, spender common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.contract.Transact(opts, "approve", spender, value)
}

// Approve is a paid mutator transaction binding the contract method 0x095ea7b3.
//
// Solidity: function approve(address spender, uint256 value) returns(bool)
func (_Contract *ContractSession) Approve(spender common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.Approve(&_Contract.TransactOpts, spender, value)
}

// Approve is a paid mutator transaction binding the contract method 0x095ea7b3.
//
// Solidity: function approve(address spender, uint256 value) returns(bool)
func (_Contract *ContractTransactorSession) Approve(spender common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.Approve(&_Contract.TransactOpts, spender, value)
}

// Burn is a paid mutator transaction binding the contract method 0x9dc29fac.
//
// Solidity: function burn(address mintAddress, uint256 amount) returns()
func (_Contract *ContractTransactor) Burn(opts *bind.TransactOpts, mintAddress common.Address, amount *big.Int) (*types.Transaction, error) {
	return _Contract.contract.Transact(opts, "burn", mintAddress, amount)
}

// Burn is a paid mutator transaction binding the contract method 0x9dc29fac.
//
// Solidity: function burn(address mintAddress, uint256 amount) returns()
func (_Contract *ContractSession) Burn(mintAddress common.Address, amount *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.Burn(&_Contract.TransactOpts, mintAddress, amount)
}

// Burn is a paid mutator transaction binding the contract method 0x9dc29fac.
//
// Solidity: function burn(address mintAddress, uint256 amount) returns()
func (_Contract *ContractTransactorSession) Burn(mintAddress common.Address, amount *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.Burn(&_Contract.TransactOpts, mintAddress, amount)
}

// Initialize is a paid mutator transaction binding the contract method 0x4571e3a6.
//
// Solidity: function initialize(address ics20_, address escrow_, string fullDenomPath_) returns()
func (_Contract *ContractTransactor) Initialize(opts *bind.TransactOpts, ics20_ common.Address, escrow_ common.Address, fullDenomPath_ string) (*types.Transaction, error) {
	return _Contract.contract.Transact(opts, "initialize", ics20_, escrow_, fullDenomPath_)
}

// Initialize is a paid mutator transaction binding the contract method 0x4571e3a6.
//
// Solidity: function initialize(address ics20_, address escrow_, string fullDenomPath_) returns()
func (_Contract *ContractSession) Initialize(ics20_ common.Address, escrow_ common.Address, fullDenomPath_ string) (*types.Transaction, error) {
	return _Contract.Contract.Initialize(&_Contract.TransactOpts, ics20_, escrow_, fullDenomPath_)
}

// Initialize is a paid mutator transaction binding the contract method 0x4571e3a6.
//
// Solidity: function initialize(address ics20_, address escrow_, string fullDenomPath_) returns()
func (_Contract *ContractTransactorSession) Initialize(ics20_ common.Address, escrow_ common.Address, fullDenomPath_ string) (*types.Transaction, error) {
	return _Contract.Contract.Initialize(&_Contract.TransactOpts, ics20_, escrow_, fullDenomPath_)
}

// Mint is a paid mutator transaction binding the contract method 0x40c10f19.
//
// Solidity: function mint(address mintAddress, uint256 amount) returns()
func (_Contract *ContractTransactor) Mint(opts *bind.TransactOpts, mintAddress common.Address, amount *big.Int) (*types.Transaction, error) {
	return _Contract.contract.Transact(opts, "mint", mintAddress, amount)
}

// Mint is a paid mutator transaction binding the contract method 0x40c10f19.
//
// Solidity: function mint(address mintAddress, uint256 amount) returns()
func (_Contract *ContractSession) Mint(mintAddress common.Address, amount *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.Mint(&_Contract.TransactOpts, mintAddress, amount)
}

// Mint is a paid mutator transaction binding the contract method 0x40c10f19.
//
// Solidity: function mint(address mintAddress, uint256 amount) returns()
func (_Contract *ContractTransactorSession) Mint(mintAddress common.Address, amount *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.Mint(&_Contract.TransactOpts, mintAddress, amount)
}

// SetMetadata is a paid mutator transaction binding the contract method 0xb090e66c.
//
// Solidity: function setMetadata(uint8 customDecimals, string customName, string customSymbol) returns()
func (_Contract *ContractTransactor) SetMetadata(opts *bind.TransactOpts, customDecimals uint8, customName string, customSymbol string) (*types.Transaction, error) {
	return _Contract.contract.Transact(opts, "setMetadata", customDecimals, customName, customSymbol)
}

// SetMetadata is a paid mutator transaction binding the contract method 0xb090e66c.
//
// Solidity: function setMetadata(uint8 customDecimals, string customName, string customSymbol) returns()
func (_Contract *ContractSession) SetMetadata(customDecimals uint8, customName string, customSymbol string) (*types.Transaction, error) {
	return _Contract.Contract.SetMetadata(&_Contract.TransactOpts, customDecimals, customName, customSymbol)
}

// SetMetadata is a paid mutator transaction binding the contract method 0xb090e66c.
//
// Solidity: function setMetadata(uint8 customDecimals, string customName, string customSymbol) returns()
func (_Contract *ContractTransactorSession) SetMetadata(customDecimals uint8, customName string, customSymbol string) (*types.Transaction, error) {
	return _Contract.Contract.SetMetadata(&_Contract.TransactOpts, customDecimals, customName, customSymbol)
}

// Transfer is a paid mutator transaction binding the contract method 0xa9059cbb.
//
// Solidity: function transfer(address to, uint256 value) returns(bool)
func (_Contract *ContractTransactor) Transfer(opts *bind.TransactOpts, to common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.contract.Transact(opts, "transfer", to, value)
}

// Transfer is a paid mutator transaction binding the contract method 0xa9059cbb.
//
// Solidity: function transfer(address to, uint256 value) returns(bool)
func (_Contract *ContractSession) Transfer(to common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.Transfer(&_Contract.TransactOpts, to, value)
}

// Transfer is a paid mutator transaction binding the contract method 0xa9059cbb.
//
// Solidity: function transfer(address to, uint256 value) returns(bool)
func (_Contract *ContractTransactorSession) Transfer(to common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.Transfer(&_Contract.TransactOpts, to, value)
}

// TransferFrom is a paid mutator transaction binding the contract method 0x23b872dd.
//
// Solidity: function transferFrom(address from, address to, uint256 value) returns(bool)
func (_Contract *ContractTransactor) TransferFrom(opts *bind.TransactOpts, from common.Address, to common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.contract.Transact(opts, "transferFrom", from, to, value)
}

// TransferFrom is a paid mutator transaction binding the contract method 0x23b872dd.
//
// Solidity: function transferFrom(address from, address to, uint256 value) returns(bool)
func (_Contract *ContractSession) TransferFrom(from common.Address, to common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.TransferFrom(&_Contract.TransactOpts, from, to, value)
}

// TransferFrom is a paid mutator transaction binding the contract method 0x23b872dd.
//
// Solidity: function transferFrom(address from, address to, uint256 value) returns(bool)
func (_Contract *ContractTransactorSession) TransferFrom(from common.Address, to common.Address, value *big.Int) (*types.Transaction, error) {
	return _Contract.Contract.TransferFrom(&_Contract.TransactOpts, from, to, value)
}

// ContractApprovalIterator is returned from FilterApproval and is used to iterate over the raw logs and unpacked data for Approval events raised by the Contract contract.
type ContractApprovalIterator struct {
	Event *ContractApproval // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *ContractApprovalIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ContractApproval)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(ContractApproval)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *ContractApprovalIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ContractApprovalIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ContractApproval represents a Approval event raised by the Contract contract.
type ContractApproval struct {
	Owner   common.Address
	Spender common.Address
	Value   *big.Int
	Raw     types.Log // Blockchain specific contextual infos
}

// FilterApproval is a free log retrieval operation binding the contract event 0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925.
//
// Solidity: event Approval(address indexed owner, address indexed spender, uint256 value)
func (_Contract *ContractFilterer) FilterApproval(opts *bind.FilterOpts, owner []common.Address, spender []common.Address) (*ContractApprovalIterator, error) {

	var ownerRule []interface{}
	for _, ownerItem := range owner {
		ownerRule = append(ownerRule, ownerItem)
	}
	var spenderRule []interface{}
	for _, spenderItem := range spender {
		spenderRule = append(spenderRule, spenderItem)
	}

	logs, sub, err := _Contract.contract.FilterLogs(opts, "Approval", ownerRule, spenderRule)
	if err != nil {
		return nil, err
	}
	return &ContractApprovalIterator{contract: _Contract.contract, event: "Approval", logs: logs, sub: sub}, nil
}

// WatchApproval is a free log subscription operation binding the contract event 0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925.
//
// Solidity: event Approval(address indexed owner, address indexed spender, uint256 value)
func (_Contract *ContractFilterer) WatchApproval(opts *bind.WatchOpts, sink chan<- *ContractApproval, owner []common.Address, spender []common.Address) (event.Subscription, error) {

	var ownerRule []interface{}
	for _, ownerItem := range owner {
		ownerRule = append(ownerRule, ownerItem)
	}
	var spenderRule []interface{}
	for _, spenderItem := range spender {
		spenderRule = append(spenderRule, spenderItem)
	}

	logs, sub, err := _Contract.contract.WatchLogs(opts, "Approval", ownerRule, spenderRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ContractApproval)
				if err := _Contract.contract.UnpackLog(event, "Approval", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseApproval is a log parse operation binding the contract event 0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925.
//
// Solidity: event Approval(address indexed owner, address indexed spender, uint256 value)
func (_Contract *ContractFilterer) ParseApproval(log types.Log) (*ContractApproval, error) {
	event := new(ContractApproval)
	if err := _Contract.contract.UnpackLog(event, "Approval", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// ContractInitializedIterator is returned from FilterInitialized and is used to iterate over the raw logs and unpacked data for Initialized events raised by the Contract contract.
type ContractInitializedIterator struct {
	Event *ContractInitialized // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *ContractInitializedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ContractInitialized)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(ContractInitialized)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *ContractInitializedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ContractInitializedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ContractInitialized represents a Initialized event raised by the Contract contract.
type ContractInitialized struct {
	Version uint64
	Raw     types.Log // Blockchain specific contextual infos
}

// FilterInitialized is a free log retrieval operation binding the contract event 0xc7f505b2f371ae2175ee4913f4499e1f2633a7b5936321eed1cdaeb6115181d2.
//
// Solidity: event Initialized(uint64 version)
func (_Contract *ContractFilterer) FilterInitialized(opts *bind.FilterOpts) (*ContractInitializedIterator, error) {

	logs, sub, err := _Contract.contract.FilterLogs(opts, "Initialized")
	if err != nil {
		return nil, err
	}
	return &ContractInitializedIterator{contract: _Contract.contract, event: "Initialized", logs: logs, sub: sub}, nil
}

// WatchInitialized is a free log subscription operation binding the contract event 0xc7f505b2f371ae2175ee4913f4499e1f2633a7b5936321eed1cdaeb6115181d2.
//
// Solidity: event Initialized(uint64 version)
func (_Contract *ContractFilterer) WatchInitialized(opts *bind.WatchOpts, sink chan<- *ContractInitialized) (event.Subscription, error) {

	logs, sub, err := _Contract.contract.WatchLogs(opts, "Initialized")
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ContractInitialized)
				if err := _Contract.contract.UnpackLog(event, "Initialized", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseInitialized is a log parse operation binding the contract event 0xc7f505b2f371ae2175ee4913f4499e1f2633a7b5936321eed1cdaeb6115181d2.
//
// Solidity: event Initialized(uint64 version)
func (_Contract *ContractFilterer) ParseInitialized(log types.Log) (*ContractInitialized, error) {
	event := new(ContractInitialized)
	if err := _Contract.contract.UnpackLog(event, "Initialized", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// ContractTransferIterator is returned from FilterTransfer and is used to iterate over the raw logs and unpacked data for Transfer events raised by the Contract contract.
type ContractTransferIterator struct {
	Event *ContractTransfer // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *ContractTransferIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ContractTransfer)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(ContractTransfer)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *ContractTransferIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ContractTransferIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ContractTransfer represents a Transfer event raised by the Contract contract.
type ContractTransfer struct {
	From  common.Address
	To    common.Address
	Value *big.Int
	Raw   types.Log // Blockchain specific contextual infos
}

// FilterTransfer is a free log retrieval operation binding the contract event 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef.
//
// Solidity: event Transfer(address indexed from, address indexed to, uint256 value)
func (_Contract *ContractFilterer) FilterTransfer(opts *bind.FilterOpts, from []common.Address, to []common.Address) (*ContractTransferIterator, error) {

	var fromRule []interface{}
	for _, fromItem := range from {
		fromRule = append(fromRule, fromItem)
	}
	var toRule []interface{}
	for _, toItem := range to {
		toRule = append(toRule, toItem)
	}

	logs, sub, err := _Contract.contract.FilterLogs(opts, "Transfer", fromRule, toRule)
	if err != nil {
		return nil, err
	}
	return &ContractTransferIterator{contract: _Contract.contract, event: "Transfer", logs: logs, sub: sub}, nil
}

// WatchTransfer is a free log subscription operation binding the contract event 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef.
//
// Solidity: event Transfer(address indexed from, address indexed to, uint256 value)
func (_Contract *ContractFilterer) WatchTransfer(opts *bind.WatchOpts, sink chan<- *ContractTransfer, from []common.Address, to []common.Address) (event.Subscription, error) {

	var fromRule []interface{}
	for _, fromItem := range from {
		fromRule = append(fromRule, fromItem)
	}
	var toRule []interface{}
	for _, toItem := range to {
		toRule = append(toRule, toItem)
	}

	logs, sub, err := _Contract.contract.WatchLogs(opts, "Transfer", fromRule, toRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ContractTransfer)
				if err := _Contract.contract.UnpackLog(event, "Transfer", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseTransfer is a log parse operation binding the contract event 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef.
//
// Solidity: event Transfer(address indexed from, address indexed to, uint256 value)
func (_Contract *ContractFilterer) ParseTransfer(log types.Log) (*ContractTransfer, error) {
	event := new(ContractTransfer)
	if err := _Contract.contract.UnpackLog(event, "Transfer", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}
