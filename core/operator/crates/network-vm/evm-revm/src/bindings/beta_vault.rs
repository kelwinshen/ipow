// @generated — DO NOT EDIT
#![allow(clippy::module_inception)]

pub use beta_vault::*;
/// This module was auto-generated with ethers-rs Abigen.
/// More information at: <https://github.com/gakonst/ethers-rs>
#[allow(
    clippy::enum_variant_names,
    clippy::too_many_arguments,
    clippy::upper_case_acronyms,
    clippy::type_complexity,
    dead_code,
    non_camel_case_types,
)]
pub mod beta_vault {
    #[allow(deprecated)]
    fn __abi() -> ::ethers::core::abi::Abi {
        ::ethers::core::abi::ethabi::Contract {
            constructor: ::core::option::Option::Some(::ethers::core::abi::ethabi::Constructor {
                inputs: ::std::vec![
                    ::ethers::core::abi::ethabi::Param {
                        name: ::std::borrow::ToOwned::to_owned("_governance"),
                        kind: ::ethers::core::abi::ethabi::ParamType::Address,
                        internal_type: ::core::option::Option::Some(
                            ::std::borrow::ToOwned::to_owned("address"),
                        ),
                    },
                    ::ethers::core::abi::ethabi::Param {
                        name: ::std::borrow::ToOwned::to_owned("_ipowHeaders"),
                        kind: ::ethers::core::abi::ethabi::ParamType::Address,
                        internal_type: ::core::option::Option::Some(
                            ::std::borrow::ToOwned::to_owned("address"),
                        ),
                    },
                    ::ethers::core::abi::ethabi::Param {
                        name: ::std::borrow::ToOwned::to_owned("_params"),
                        kind: ::ethers::core::abi::ethabi::ParamType::Tuple(
                            ::std::vec![
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(16usize),
                            ],
                        ),
                        internal_type: ::core::option::Option::Some(
                            ::std::borrow::ToOwned::to_owned("struct BetaVault.Params"),
                        ),
                    },
                ],
            }),
            functions: ::core::convert::From::from([
                (
                    ::std::borrow::ToOwned::to_owned("ANCHOR_VERSION"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("ANCHOR_VERSION"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("BPS_DENOM"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("BPS_DENOM"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("KIND_ALIVE"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("KIND_ALIVE"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("KIND_ATTEST"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("KIND_ATTEST"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("KIND_CANCEL"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("KIND_CANCEL"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("KIND_CLEAR"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("KIND_CLEAR"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("KIND_MINT"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("KIND_MINT"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("KIND_RELEASE"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("KIND_RELEASE"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("KIND_VETO"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("KIND_VETO"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("anchors"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("anchors"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("kind"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("status"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned(
                                            "enum BetaVault.AnchorStatus",
                                        ),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("statementHash"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("blockHeight"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("processedAt"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("lockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("to"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("units"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("challengeUntil"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("held"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("attester"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("escrow"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("paid"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("settled"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("approveOperator"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("approveOperator"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("owner"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("approvedOperators"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("approvedOperators"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("deposit"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("deposit"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("token"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("solUser"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("nonce"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("units"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("deadline"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("lockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::Payable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("executeRelease"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("executeRelease"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("txidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("fundRewards"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("fundRewards"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::Payable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("governance"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("governance"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("insurance"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("insurance"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("insuranceReserved"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("insuranceReserved"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("ipowHeaders"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("ipowHeaders"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned(
                                            "contract IIPoWHeadersView",
                                        ),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("locks"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("locks"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("solUser"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("nonce"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("units"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("deadline"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("state"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("enum BetaVault.LockState"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("depositor"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("token"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("amount"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("mintAttester"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("mintAttester"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("nextLockId"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("nextLockId"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("params"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("params"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("ethWeiPerUnit"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("tFinSecs"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("tChallengeSecs"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("tSkipSecs"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("refundMarginSecs"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("unbondDelaySecs"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("minOperatorBond"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("minAuditorBond"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("vetoSlashWei"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("vetoRewardWei"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("bountyBps"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(16usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint16"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("parties"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("parties"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("exists"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("owner"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("kind"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("enum BetaVault.PartyKind"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("anchorTxidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("anchorVout"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(32usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("seq"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("bond"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("dead"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("unbondRequestedAt"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("paused"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("paused"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("processAnchor"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("processAnchor"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("statement"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bytes,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("txRaw"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bytes,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("blockHeight"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("branchLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Array(
                                        ::std::boxed::Box::new(
                                            ::ethers::core::abi::ethabi::ParamType::FixedBytes(32usize),
                                        ),
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32[]"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("index"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("refund"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("refund"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("lockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("registerParty"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("registerParty"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("kind"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("enum BetaVault.PartyKind"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("anchorTxidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("anchorVout"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(32usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::Payable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("requestUnbond"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("requestUnbond"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("rewardPool"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("rewardPool"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("setParams"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("setParams"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("p"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Tuple(
                                        ::std::vec![
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(16usize),
                                        ],
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("struct BetaVault.Params"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("_paused"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("setTokenParams"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("setTokenParams"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("token"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("p"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Tuple(
                                        ::std::vec![
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                        ],
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned(
                                            "struct BetaVault.TokenParams",
                                        ),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("settleRelease"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("settleRelease"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("txidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("skipAnchor"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("skipAnchor"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("txRaw"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bytes,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("blockHeight"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("branchLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Array(
                                        ::std::boxed::Box::new(
                                            ::ethers::core::abi::ethabi::ParamType::FixedBytes(32usize),
                                        ),
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32[]"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("index"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("tokenParams"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("tokenParams"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("amountPerUnit"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("slashWeiPerUnit"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("topUpBond"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("topUpBond"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::Payable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("totalBonds"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("totalBonds"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("totalLocked"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("totalLocked"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("withdrawBond"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("withdrawBond"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
            ]),
            events: ::core::convert::From::from([
                (
                    ::std::borrow::ToOwned::to_owned("AnchorProcessed"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("AnchorProcessed"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("txidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("kind"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("status"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("Deposited"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("Deposited"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("lockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("solUser"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("nonce"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("units"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("deadline"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("depositor"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("token"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("Finalized"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("Finalized"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("lockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("anchorTxidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("PartyRegistered"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("PartyRegistered"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("kind"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("owner"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("bond"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("Refunded"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("Refunded"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("lockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: true,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("ReleaseHeld"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("ReleaseHeld"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("txidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("held"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("ReleasePaidFromEscrow"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned(
                                "ReleasePaidFromEscrow",
                            ),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("txidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("attester"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("to"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("wei_"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("ReleaseQueued"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("ReleaseQueued"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("txidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("lockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("to"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("units"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("releaseAfter"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("ReleaseSettled"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("ReleaseSettled"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("txidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("reimbursed"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("Released"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("Released"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("txidLE"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("lockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("to"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("wei_"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("Slashed"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("Slashed"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("partyId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("amount"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("submitter"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("TokenParamsSet"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("TokenParamsSet"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("token"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("amountPerUnit"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("slashWeiPerUnit"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
            ]),
            errors: ::core::convert::From::from([
                (
                    ::std::borrow::ToOwned::to_owned("AlreadyProcessed"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("AlreadyProcessed"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("BadAnchorPayload"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("BadAnchorPayload"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("BadAnchorState"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("BadAnchorState"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("BadLockState"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("BadLockState"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("BondTooSmall"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("BondTooSmall"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("Held"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("Held"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("InvalidHeader"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("InvalidHeader"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("InvalidMerkleBranch"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "InvalidMerkleBranch",
                            ),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("InvalidParams"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("InvalidParams"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("KindMismatch"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("KindMismatch"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("MalformedStatement"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("MalformedStatement"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("MalformedTx"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("MalformedTx"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("NoParty"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NoParty"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("NotApproved"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NotApproved"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("NotAuditor"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NotAuditor"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("NotOnStatementChain"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "NotOnStatementChain",
                            ),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("NotOperator"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NotOperator"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("NotReady"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NotReady"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("PartyDead"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("PartyDead"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("PartyExists"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("PartyExists"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("Paused"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("Paused"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("RateLimited"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("RateLimited"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("ReentrancyGuardReentrantCall"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "ReentrancyGuardReentrantCall",
                            ),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("RefundNotReady"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("RefundNotReady"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("ReleaseTokenUnsupported"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "ReleaseTokenUnsupported",
                            ),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("SafeERC20FailedOperation"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "SafeERC20FailedOperation",
                            ),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("token"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                            ],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("SkipNotReady"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("SkipNotReady"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("StatementHashMismatch"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "StatementHashMismatch",
                            ),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("TargetNotProcessed"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("TargetNotProcessed"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("TokenNotRegistered"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("TokenNotRegistered"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("TransferFailed"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("TransferFailed"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("TxidMismatch"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("TxidMismatch"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("Unauthorized"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("Unauthorized"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("UnbondNotReady"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("UnbondNotReady"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("UnbondNotRequested"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("UnbondNotRequested"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("WitnessSerialization"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "WitnessSerialization",
                            ),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
            ]),
            receive: true,
            fallback: false,
        }
    }
    ///The parsed JSON ABI of the contract.
    pub static BETAVAULT_ABI: ::ethers::contract::Lazy<::ethers::core::abi::Abi> = ::ethers::contract::Lazy::new(
        __abi,
    );
    pub struct BetaVault<M>(::ethers::contract::Contract<M>);
    impl<M> ::core::clone::Clone for BetaVault<M> {
        fn clone(&self) -> Self {
            Self(::core::clone::Clone::clone(&self.0))
        }
    }
    impl<M> ::core::ops::Deref for BetaVault<M> {
        type Target = ::ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> ::core::ops::DerefMut for BetaVault<M> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl<M> ::core::fmt::Debug for BetaVault<M> {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple(::core::stringify!(BetaVault)).field(&self.address()).finish()
        }
    }
    impl<M: ::ethers::providers::Middleware> BetaVault<M> {
        /// Creates a new contract instance with the specified `ethers` client at
        /// `address`. The contract derefs to a `ethers::Contract` object.
        pub fn new<T: Into<::ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            Self(
                ::ethers::contract::Contract::new(
                    address.into(),
                    BETAVAULT_ABI.clone(),
                    client,
                ),
            )
        }
        ///Calls the contract's `ANCHOR_VERSION` (0x2d998d18) function
        pub fn anchor_version(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, u8> {
            self.0
                .method_hash([45, 153, 141, 24], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `BPS_DENOM` (0x6637e38c) function
        pub fn bps_denom(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([102, 55, 227, 140], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `KIND_ALIVE` (0x8649ff4c) function
        pub fn kind_alive(&self) -> ::ethers::contract::builders::ContractCall<M, u8> {
            self.0
                .method_hash([134, 73, 255, 76], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `KIND_ATTEST` (0x388043f4) function
        pub fn kind_attest(&self) -> ::ethers::contract::builders::ContractCall<M, u8> {
            self.0
                .method_hash([56, 128, 67, 244], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `KIND_CANCEL` (0xc4b8d3f9) function
        pub fn kind_cancel(&self) -> ::ethers::contract::builders::ContractCall<M, u8> {
            self.0
                .method_hash([196, 184, 211, 249], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `KIND_CLEAR` (0x232b88c1) function
        pub fn kind_clear(&self) -> ::ethers::contract::builders::ContractCall<M, u8> {
            self.0
                .method_hash([35, 43, 136, 193], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `KIND_MINT` (0x81d2f871) function
        pub fn kind_mint(&self) -> ::ethers::contract::builders::ContractCall<M, u8> {
            self.0
                .method_hash([129, 210, 248, 113], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `KIND_RELEASE` (0xeab1f2c6) function
        pub fn kind_release(&self) -> ::ethers::contract::builders::ContractCall<M, u8> {
            self.0
                .method_hash([234, 177, 242, 198], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `KIND_VETO` (0xfb7e66b3) function
        pub fn kind_veto(&self) -> ::ethers::contract::builders::ContractCall<M, u8> {
            self.0
                .method_hash([251, 126, 102, 179], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `anchors` (0xb01b6d53) function
        pub fn anchors(
            &self,
            p0: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            (
                [u8; 32],
                u8,
                u8,
                [u8; 32],
                u64,
                u64,
                u64,
                ::ethers::core::types::Address,
                u64,
                u64,
                bool,
                [u8; 32],
                ::ethers::core::types::U256,
                bool,
                bool,
            ),
        > {
            self.0
                .method_hash([176, 27, 109, 83], p0)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `approveOperator` (0xfebbaac2) function
        pub fn approve_operator(
            &self,
            party_id: [u8; 32],
            owner: ::ethers::core::types::Address,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([254, 187, 170, 194], (party_id, owner))
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `approvedOperators` (0x677dd834) function
        pub fn approved_operators(
            &self,
            p0: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            ::ethers::core::types::Address,
        > {
            self.0
                .method_hash([103, 125, 216, 52], p0)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `deposit` (0xe039676a) function
        pub fn deposit(
            &self,
            token: ::ethers::core::types::Address,
            sol_user: [u8; 32],
            nonce: u64,
            units: u64,
            deadline: u64,
        ) -> ::ethers::contract::builders::ContractCall<M, u64> {
            self.0
                .method_hash(
                    [224, 57, 103, 106],
                    (token, sol_user, nonce, units, deadline),
                )
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `executeRelease` (0xc799147b) function
        pub fn execute_release(
            &self,
            txid_le: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([199, 153, 20, 123], txid_le)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `fundRewards` (0xff18bf0b) function
        pub fn fund_rewards(&self) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([255, 24, 191, 11], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `governance` (0x5aa6e675) function
        pub fn governance(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            ::ethers::core::types::Address,
        > {
            self.0
                .method_hash([90, 166, 230, 117], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `insurance` (0x89cf3204) function
        pub fn insurance(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([137, 207, 50, 4], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `insuranceReserved` (0x28cfdafe) function
        pub fn insurance_reserved(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([40, 207, 218, 254], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `ipowHeaders` (0x21d5dc2f) function
        pub fn ipow_headers(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            ::ethers::core::types::Address,
        > {
            self.0
                .method_hash([33, 213, 220, 47], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `locks` (0x26043c0e) function
        pub fn locks(
            &self,
            p0: u64,
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            (
                [u8; 32],
                u64,
                u64,
                u64,
                u8,
                ::ethers::core::types::Address,
                ::ethers::core::types::Address,
                ::ethers::core::types::U256,
            ),
        > {
            self.0
                .method_hash([38, 4, 60, 14], p0)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `mintAttester` (0x0f55b6bf) function
        pub fn mint_attester(
            &self,
            p0: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, [u8; 32]> {
            self.0
                .method_hash([15, 85, 182, 191], p0)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `nextLockId` (0x6518a0b3) function
        pub fn next_lock_id(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, u64> {
            self.0
                .method_hash([101, 24, 160, 179], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `params` (0xcff0ab96) function
        pub fn params(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            (
                ::ethers::core::types::U256,
                u64,
                u64,
                u64,
                u64,
                u64,
                ::ethers::core::types::U256,
                ::ethers::core::types::U256,
                ::ethers::core::types::U256,
                ::ethers::core::types::U256,
                u16,
            ),
        > {
            self.0
                .method_hash([207, 240, 171, 150], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `parties` (0x941a581c) function
        pub fn parties(
            &self,
            p0: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            (
                bool,
                ::ethers::core::types::Address,
                u8,
                [u8; 32],
                u32,
                u64,
                ::ethers::core::types::U256,
                bool,
                u64,
            ),
        > {
            self.0
                .method_hash([148, 26, 88, 28], p0)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `paused` (0x5c975abb) function
        pub fn paused(&self) -> ::ethers::contract::builders::ContractCall<M, bool> {
            self.0
                .method_hash([92, 151, 90, 187], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `processAnchor` (0xc7635b80) function
        pub fn process_anchor(
            &self,
            party_id: [u8; 32],
            statement: ::ethers::core::types::Bytes,
            tx_raw: ::ethers::core::types::Bytes,
            block_height: ::ethers::core::types::U256,
            branch_le: ::std::vec::Vec<[u8; 32]>,
            index: ::ethers::core::types::U256,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash(
                    [199, 99, 91, 128],
                    (party_id, statement, tx_raw, block_height, branch_le, index),
                )
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `refund` (0xd7194ccb) function
        pub fn refund(
            &self,
            lock_id: u64,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([215, 25, 76, 203], lock_id)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `registerParty` (0x58772a1e) function
        pub fn register_party(
            &self,
            party_id: [u8; 32],
            kind: u8,
            anchor_txid_le: [u8; 32],
            anchor_vout: u32,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash(
                    [88, 119, 42, 30],
                    (party_id, kind, anchor_txid_le, anchor_vout),
                )
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `requestUnbond` (0x977d178f) function
        pub fn request_unbond(
            &self,
            party_id: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([151, 125, 23, 143], party_id)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `rewardPool` (0x66666aa9) function
        pub fn reward_pool(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([102, 102, 106, 169], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `setParams` (0xa9998a5f) function
        pub fn set_params(
            &self,
            p: Params,
            paused: bool,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([169, 153, 138, 95], (p, paused))
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `setTokenParams` (0x4fd6eca7) function
        pub fn set_token_params(
            &self,
            token: ::ethers::core::types::Address,
            p: TokenParams,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([79, 214, 236, 167], (token, p))
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `settleRelease` (0x2acb293b) function
        pub fn settle_release(
            &self,
            txid_le: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([42, 203, 41, 59], txid_le)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `skipAnchor` (0xad1d0e8e) function
        pub fn skip_anchor(
            &self,
            party_id: [u8; 32],
            tx_raw: ::ethers::core::types::Bytes,
            block_height: ::ethers::core::types::U256,
            branch_le: ::std::vec::Vec<[u8; 32]>,
            index: ::ethers::core::types::U256,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash(
                    [173, 29, 14, 142],
                    (party_id, tx_raw, block_height, branch_le, index),
                )
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `tokenParams` (0x012374c9) function
        pub fn token_params(
            &self,
            p0: ::ethers::core::types::Address,
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            (::ethers::core::types::U256, ::ethers::core::types::U256),
        > {
            self.0
                .method_hash([1, 35, 116, 201], p0)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `topUpBond` (0x1cb8fed9) function
        pub fn top_up_bond(
            &self,
            party_id: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([28, 184, 254, 217], party_id)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `totalBonds` (0xf263c470) function
        pub fn total_bonds(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([242, 99, 196, 112], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `totalLocked` (0xd8fb9337) function
        pub fn total_locked(
            &self,
            p0: ::ethers::core::types::Address,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([216, 251, 147, 55], p0)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `withdrawBond` (0x7285e1ac) function
        pub fn withdraw_bond(
            &self,
            party_id: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([114, 133, 225, 172], party_id)
                .expect("method not found (this should never happen)")
        }
        ///Gets the contract's `AnchorProcessed` event
        pub fn anchor_processed_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            AnchorProcessedFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `Deposited` event
        pub fn deposited_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            DepositedFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `Finalized` event
        pub fn finalized_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            FinalizedFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `PartyRegistered` event
        pub fn party_registered_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            PartyRegisteredFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `Refunded` event
        pub fn refunded_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            RefundedFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `ReleaseHeld` event
        pub fn release_held_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            ReleaseHeldFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `ReleasePaidFromEscrow` event
        pub fn release_paid_from_escrow_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            ReleasePaidFromEscrowFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `ReleaseQueued` event
        pub fn release_queued_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            ReleaseQueuedFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `ReleaseSettled` event
        pub fn release_settled_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            ReleaseSettledFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `Released` event
        pub fn released_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            ReleasedFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `Slashed` event
        pub fn slashed_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<::std::sync::Arc<M>, M, SlashedFilter> {
            self.0.event()
        }
        ///Gets the contract's `TokenParamsSet` event
        pub fn token_params_set_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            TokenParamsSetFilter,
        > {
            self.0.event()
        }
        /// Returns an `Event` builder for all the events of this contract.
        pub fn events(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            BetaVaultEvents,
        > {
            self.0.event_with_filter(::core::default::Default::default())
        }
    }
    impl<M: ::ethers::providers::Middleware> From<::ethers::contract::Contract<M>>
    for BetaVault<M> {
        fn from(contract: ::ethers::contract::Contract<M>) -> Self {
            Self::new(contract.address(), contract.client())
        }
    }
    ///Custom Error type `AlreadyProcessed` with signature `AlreadyProcessed()` and selector `0x57eee766`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "AlreadyProcessed", abi = "AlreadyProcessed()")]
    pub struct AlreadyProcessed;
    ///Custom Error type `BadAnchorPayload` with signature `BadAnchorPayload()` and selector `0x12b290ea`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "BadAnchorPayload", abi = "BadAnchorPayload()")]
    pub struct BadAnchorPayload;
    ///Custom Error type `BadAnchorState` with signature `BadAnchorState()` and selector `0xf0bb9d8d`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "BadAnchorState", abi = "BadAnchorState()")]
    pub struct BadAnchorState;
    ///Custom Error type `BadLockState` with signature `BadLockState()` and selector `0xa278f1e2`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "BadLockState", abi = "BadLockState()")]
    pub struct BadLockState;
    ///Custom Error type `BondTooSmall` with signature `BondTooSmall()` and selector `0x3388f4fc`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "BondTooSmall", abi = "BondTooSmall()")]
    pub struct BondTooSmall;
    ///Custom Error type `Held` with signature `Held()` and selector `0x7ce0aae6`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "Held", abi = "Held()")]
    pub struct Held;
    ///Custom Error type `InvalidHeader` with signature `InvalidHeader()` and selector `0xbabb01dd`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "InvalidHeader", abi = "InvalidHeader()")]
    pub struct InvalidHeader;
    ///Custom Error type `InvalidMerkleBranch` with signature `InvalidMerkleBranch()` and selector `0xc8e75319`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "InvalidMerkleBranch", abi = "InvalidMerkleBranch()")]
    pub struct InvalidMerkleBranch;
    ///Custom Error type `InvalidParams` with signature `InvalidParams()` and selector `0xa86b6512`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "InvalidParams", abi = "InvalidParams()")]
    pub struct InvalidParams;
    ///Custom Error type `KindMismatch` with signature `KindMismatch()` and selector `0x8947714b`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "KindMismatch", abi = "KindMismatch()")]
    pub struct KindMismatch;
    ///Custom Error type `MalformedStatement` with signature `MalformedStatement()` and selector `0x688bd86b`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "MalformedStatement", abi = "MalformedStatement()")]
    pub struct MalformedStatement;
    ///Custom Error type `MalformedTx` with signature `MalformedTx()` and selector `0xa58c586a`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "MalformedTx", abi = "MalformedTx()")]
    pub struct MalformedTx;
    ///Custom Error type `NoParty` with signature `NoParty()` and selector `0xc3fae8de`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "NoParty", abi = "NoParty()")]
    pub struct NoParty;
    ///Custom Error type `NotApproved` with signature `NotApproved()` and selector `0xc19f17a9`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "NotApproved", abi = "NotApproved()")]
    pub struct NotApproved;
    ///Custom Error type `NotAuditor` with signature `NotAuditor()` and selector `0x5d5a323c`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "NotAuditor", abi = "NotAuditor()")]
    pub struct NotAuditor;
    ///Custom Error type `NotOnStatementChain` with signature `NotOnStatementChain()` and selector `0x080e25fa`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "NotOnStatementChain", abi = "NotOnStatementChain()")]
    pub struct NotOnStatementChain;
    ///Custom Error type `NotOperator` with signature `NotOperator()` and selector `0x7c214f04`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "NotOperator", abi = "NotOperator()")]
    pub struct NotOperator;
    ///Custom Error type `NotReady` with signature `NotReady()` and selector `0x9488aaa6`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "NotReady", abi = "NotReady()")]
    pub struct NotReady;
    ///Custom Error type `PartyDead` with signature `PartyDead()` and selector `0xbc329edd`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "PartyDead", abi = "PartyDead()")]
    pub struct PartyDead;
    ///Custom Error type `PartyExists` with signature `PartyExists()` and selector `0x9e2ca66a`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "PartyExists", abi = "PartyExists()")]
    pub struct PartyExists;
    ///Custom Error type `Paused` with signature `Paused()` and selector `0x9e87fac8`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "Paused", abi = "Paused()")]
    pub struct Paused;
    ///Custom Error type `RateLimited` with signature `RateLimited()` and selector `0x3f7b7a68`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "RateLimited", abi = "RateLimited()")]
    pub struct RateLimited;
    ///Custom Error type `ReentrancyGuardReentrantCall` with signature `ReentrancyGuardReentrantCall()` and selector `0x3ee5aeb5`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(
        name = "ReentrancyGuardReentrantCall",
        abi = "ReentrancyGuardReentrantCall()"
    )]
    pub struct ReentrancyGuardReentrantCall;
    ///Custom Error type `RefundNotReady` with signature `RefundNotReady()` and selector `0xc7a4f4a2`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "RefundNotReady", abi = "RefundNotReady()")]
    pub struct RefundNotReady;
    ///Custom Error type `ReleaseTokenUnsupported` with signature `ReleaseTokenUnsupported()` and selector `0xc28b8e43`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "ReleaseTokenUnsupported", abi = "ReleaseTokenUnsupported()")]
    pub struct ReleaseTokenUnsupported;
    ///Custom Error type `SafeERC20FailedOperation` with signature `SafeERC20FailedOperation(address)` and selector `0x5274afe7`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(
        name = "SafeERC20FailedOperation",
        abi = "SafeERC20FailedOperation(address)"
    )]
    pub struct SafeERC20FailedOperation {
        pub token: ::ethers::core::types::Address,
    }
    ///Custom Error type `SkipNotReady` with signature `SkipNotReady()` and selector `0x0ba1aa1b`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "SkipNotReady", abi = "SkipNotReady()")]
    pub struct SkipNotReady;
    ///Custom Error type `StatementHashMismatch` with signature `StatementHashMismatch()` and selector `0x9568fd42`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "StatementHashMismatch", abi = "StatementHashMismatch()")]
    pub struct StatementHashMismatch;
    ///Custom Error type `TargetNotProcessed` with signature `TargetNotProcessed()` and selector `0x1692c428`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "TargetNotProcessed", abi = "TargetNotProcessed()")]
    pub struct TargetNotProcessed;
    ///Custom Error type `TokenNotRegistered` with signature `TokenNotRegistered()` and selector `0x259ba1ad`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "TokenNotRegistered", abi = "TokenNotRegistered()")]
    pub struct TokenNotRegistered;
    ///Custom Error type `TransferFailed` with signature `TransferFailed()` and selector `0x90b8ec18`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "TransferFailed", abi = "TransferFailed()")]
    pub struct TransferFailed;
    ///Custom Error type `TxidMismatch` with signature `TxidMismatch()` and selector `0x43915a4e`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "TxidMismatch", abi = "TxidMismatch()")]
    pub struct TxidMismatch;
    ///Custom Error type `Unauthorized` with signature `Unauthorized()` and selector `0x82b42900`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "Unauthorized", abi = "Unauthorized()")]
    pub struct Unauthorized;
    ///Custom Error type `UnbondNotReady` with signature `UnbondNotReady()` and selector `0xa2ef86a9`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "UnbondNotReady", abi = "UnbondNotReady()")]
    pub struct UnbondNotReady;
    ///Custom Error type `UnbondNotRequested` with signature `UnbondNotRequested()` and selector `0xd279f289`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "UnbondNotRequested", abi = "UnbondNotRequested()")]
    pub struct UnbondNotRequested;
    ///Custom Error type `WitnessSerialization` with signature `WitnessSerialization()` and selector `0x0c212935`
    #[derive(
        Clone,
        ::ethers::contract::EthError,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[etherror(name = "WitnessSerialization", abi = "WitnessSerialization()")]
    pub struct WitnessSerialization;
    ///Container type for all of the contract's custom errors
    #[derive(Clone, ::ethers::contract::EthAbiType, Debug, PartialEq, Eq, Hash)]
    pub enum BetaVaultErrors {
        AlreadyProcessed(AlreadyProcessed),
        BadAnchorPayload(BadAnchorPayload),
        BadAnchorState(BadAnchorState),
        BadLockState(BadLockState),
        BondTooSmall(BondTooSmall),
        Held(Held),
        InvalidHeader(InvalidHeader),
        InvalidMerkleBranch(InvalidMerkleBranch),
        InvalidParams(InvalidParams),
        KindMismatch(KindMismatch),
        MalformedStatement(MalformedStatement),
        MalformedTx(MalformedTx),
        NoParty(NoParty),
        NotApproved(NotApproved),
        NotAuditor(NotAuditor),
        NotOnStatementChain(NotOnStatementChain),
        NotOperator(NotOperator),
        NotReady(NotReady),
        PartyDead(PartyDead),
        PartyExists(PartyExists),
        Paused(Paused),
        RateLimited(RateLimited),
        ReentrancyGuardReentrantCall(ReentrancyGuardReentrantCall),
        RefundNotReady(RefundNotReady),
        ReleaseTokenUnsupported(ReleaseTokenUnsupported),
        SafeERC20FailedOperation(SafeERC20FailedOperation),
        SkipNotReady(SkipNotReady),
        StatementHashMismatch(StatementHashMismatch),
        TargetNotProcessed(TargetNotProcessed),
        TokenNotRegistered(TokenNotRegistered),
        TransferFailed(TransferFailed),
        TxidMismatch(TxidMismatch),
        Unauthorized(Unauthorized),
        UnbondNotReady(UnbondNotReady),
        UnbondNotRequested(UnbondNotRequested),
        WitnessSerialization(WitnessSerialization),
        /// The standard solidity revert string, with selector
        /// Error(string) -- 0x08c379a0
        RevertString(::std::string::String),
    }
    impl ::ethers::core::abi::AbiDecode for BetaVaultErrors {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::core::result::Result<Self, ::ethers::core::abi::AbiError> {
            let data = data.as_ref();
            if let Ok(decoded) = <::std::string::String as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RevertString(decoded));
            }
            if let Ok(decoded) = <AlreadyProcessed as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::AlreadyProcessed(decoded));
            }
            if let Ok(decoded) = <BadAnchorPayload as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::BadAnchorPayload(decoded));
            }
            if let Ok(decoded) = <BadAnchorState as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::BadAnchorState(decoded));
            }
            if let Ok(decoded) = <BadLockState as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::BadLockState(decoded));
            }
            if let Ok(decoded) = <BondTooSmall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::BondTooSmall(decoded));
            }
            if let Ok(decoded) = <Held as ::ethers::core::abi::AbiDecode>::decode(data) {
                return Ok(Self::Held(decoded));
            }
            if let Ok(decoded) = <InvalidHeader as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::InvalidHeader(decoded));
            }
            if let Ok(decoded) = <InvalidMerkleBranch as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::InvalidMerkleBranch(decoded));
            }
            if let Ok(decoded) = <InvalidParams as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::InvalidParams(decoded));
            }
            if let Ok(decoded) = <KindMismatch as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::KindMismatch(decoded));
            }
            if let Ok(decoded) = <MalformedStatement as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::MalformedStatement(decoded));
            }
            if let Ok(decoded) = <MalformedTx as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::MalformedTx(decoded));
            }
            if let Ok(decoded) = <NoParty as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NoParty(decoded));
            }
            if let Ok(decoded) = <NotApproved as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NotApproved(decoded));
            }
            if let Ok(decoded) = <NotAuditor as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NotAuditor(decoded));
            }
            if let Ok(decoded) = <NotOnStatementChain as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NotOnStatementChain(decoded));
            }
            if let Ok(decoded) = <NotOperator as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NotOperator(decoded));
            }
            if let Ok(decoded) = <NotReady as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NotReady(decoded));
            }
            if let Ok(decoded) = <PartyDead as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::PartyDead(decoded));
            }
            if let Ok(decoded) = <PartyExists as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::PartyExists(decoded));
            }
            if let Ok(decoded) = <Paused as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Paused(decoded));
            }
            if let Ok(decoded) = <RateLimited as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RateLimited(decoded));
            }
            if let Ok(decoded) = <ReentrancyGuardReentrantCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ReentrancyGuardReentrantCall(decoded));
            }
            if let Ok(decoded) = <RefundNotReady as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RefundNotReady(decoded));
            }
            if let Ok(decoded) = <ReleaseTokenUnsupported as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ReleaseTokenUnsupported(decoded));
            }
            if let Ok(decoded) = <SafeERC20FailedOperation as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SafeERC20FailedOperation(decoded));
            }
            if let Ok(decoded) = <SkipNotReady as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SkipNotReady(decoded));
            }
            if let Ok(decoded) = <StatementHashMismatch as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::StatementHashMismatch(decoded));
            }
            if let Ok(decoded) = <TargetNotProcessed as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TargetNotProcessed(decoded));
            }
            if let Ok(decoded) = <TokenNotRegistered as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TokenNotRegistered(decoded));
            }
            if let Ok(decoded) = <TransferFailed as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TransferFailed(decoded));
            }
            if let Ok(decoded) = <TxidMismatch as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TxidMismatch(decoded));
            }
            if let Ok(decoded) = <Unauthorized as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Unauthorized(decoded));
            }
            if let Ok(decoded) = <UnbondNotReady as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::UnbondNotReady(decoded));
            }
            if let Ok(decoded) = <UnbondNotRequested as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::UnbondNotRequested(decoded));
            }
            if let Ok(decoded) = <WitnessSerialization as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::WitnessSerialization(decoded));
            }
            Err(::ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ::ethers::core::abi::AbiEncode for BetaVaultErrors {
        fn encode(self) -> ::std::vec::Vec<u8> {
            match self {
                Self::AlreadyProcessed(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::BadAnchorPayload(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::BadAnchorState(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::BadLockState(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::BondTooSmall(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Held(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::InvalidHeader(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::InvalidMerkleBranch(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::InvalidParams(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::KindMismatch(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::MalformedStatement(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::MalformedTx(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::NoParty(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::NotApproved(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::NotAuditor(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::NotOnStatementChain(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::NotOperator(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::NotReady(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::PartyDead(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::PartyExists(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Paused(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::RateLimited(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ReentrancyGuardReentrantCall(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::RefundNotReady(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ReleaseTokenUnsupported(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SafeERC20FailedOperation(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SkipNotReady(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::StatementHashMismatch(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TargetNotProcessed(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TokenNotRegistered(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TransferFailed(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TxidMismatch(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Unauthorized(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::UnbondNotReady(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::UnbondNotRequested(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::WitnessSerialization(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::RevertString(s) => ::ethers::core::abi::AbiEncode::encode(s),
            }
        }
    }
    impl ::ethers::contract::ContractRevert for BetaVaultErrors {
        fn valid_selector(selector: [u8; 4]) -> bool {
            match selector {
                [0x08, 0xc3, 0x79, 0xa0] => true,
                _ if selector
                    == <AlreadyProcessed as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <BadAnchorPayload as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <BadAnchorState as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <BadLockState as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <BondTooSmall as ::ethers::contract::EthError>::selector() => true,
                _ if selector == <Held as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <InvalidHeader as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <InvalidMerkleBranch as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <InvalidParams as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <KindMismatch as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <MalformedStatement as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <MalformedTx as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <NoParty as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <NotApproved as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <NotAuditor as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <NotOnStatementChain as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <NotOperator as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <NotReady as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <PartyDead as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <PartyExists as ::ethers::contract::EthError>::selector() => true,
                _ if selector == <Paused as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <RateLimited as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <ReentrancyGuardReentrantCall as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <RefundNotReady as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <ReleaseTokenUnsupported as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <SafeERC20FailedOperation as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <SkipNotReady as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <StatementHashMismatch as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <TargetNotProcessed as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <TokenNotRegistered as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <TransferFailed as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <TxidMismatch as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <Unauthorized as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <UnbondNotReady as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <UnbondNotRequested as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <WitnessSerialization as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ => false,
            }
        }
    }
    impl ::core::fmt::Display for BetaVaultErrors {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            match self {
                Self::AlreadyProcessed(element) => ::core::fmt::Display::fmt(element, f),
                Self::BadAnchorPayload(element) => ::core::fmt::Display::fmt(element, f),
                Self::BadAnchorState(element) => ::core::fmt::Display::fmt(element, f),
                Self::BadLockState(element) => ::core::fmt::Display::fmt(element, f),
                Self::BondTooSmall(element) => ::core::fmt::Display::fmt(element, f),
                Self::Held(element) => ::core::fmt::Display::fmt(element, f),
                Self::InvalidHeader(element) => ::core::fmt::Display::fmt(element, f),
                Self::InvalidMerkleBranch(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::InvalidParams(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindMismatch(element) => ::core::fmt::Display::fmt(element, f),
                Self::MalformedStatement(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::MalformedTx(element) => ::core::fmt::Display::fmt(element, f),
                Self::NoParty(element) => ::core::fmt::Display::fmt(element, f),
                Self::NotApproved(element) => ::core::fmt::Display::fmt(element, f),
                Self::NotAuditor(element) => ::core::fmt::Display::fmt(element, f),
                Self::NotOnStatementChain(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::NotOperator(element) => ::core::fmt::Display::fmt(element, f),
                Self::NotReady(element) => ::core::fmt::Display::fmt(element, f),
                Self::PartyDead(element) => ::core::fmt::Display::fmt(element, f),
                Self::PartyExists(element) => ::core::fmt::Display::fmt(element, f),
                Self::Paused(element) => ::core::fmt::Display::fmt(element, f),
                Self::RateLimited(element) => ::core::fmt::Display::fmt(element, f),
                Self::ReentrancyGuardReentrantCall(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::RefundNotReady(element) => ::core::fmt::Display::fmt(element, f),
                Self::ReleaseTokenUnsupported(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::SafeERC20FailedOperation(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::SkipNotReady(element) => ::core::fmt::Display::fmt(element, f),
                Self::StatementHashMismatch(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::TargetNotProcessed(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::TokenNotRegistered(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::TransferFailed(element) => ::core::fmt::Display::fmt(element, f),
                Self::TxidMismatch(element) => ::core::fmt::Display::fmt(element, f),
                Self::Unauthorized(element) => ::core::fmt::Display::fmt(element, f),
                Self::UnbondNotReady(element) => ::core::fmt::Display::fmt(element, f),
                Self::UnbondNotRequested(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::WitnessSerialization(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::RevertString(s) => ::core::fmt::Display::fmt(s, f),
            }
        }
    }
    impl ::core::convert::From<::std::string::String> for BetaVaultErrors {
        fn from(value: String) -> Self {
            Self::RevertString(value)
        }
    }
    impl ::core::convert::From<AlreadyProcessed> for BetaVaultErrors {
        fn from(value: AlreadyProcessed) -> Self {
            Self::AlreadyProcessed(value)
        }
    }
    impl ::core::convert::From<BadAnchorPayload> for BetaVaultErrors {
        fn from(value: BadAnchorPayload) -> Self {
            Self::BadAnchorPayload(value)
        }
    }
    impl ::core::convert::From<BadAnchorState> for BetaVaultErrors {
        fn from(value: BadAnchorState) -> Self {
            Self::BadAnchorState(value)
        }
    }
    impl ::core::convert::From<BadLockState> for BetaVaultErrors {
        fn from(value: BadLockState) -> Self {
            Self::BadLockState(value)
        }
    }
    impl ::core::convert::From<BondTooSmall> for BetaVaultErrors {
        fn from(value: BondTooSmall) -> Self {
            Self::BondTooSmall(value)
        }
    }
    impl ::core::convert::From<Held> for BetaVaultErrors {
        fn from(value: Held) -> Self {
            Self::Held(value)
        }
    }
    impl ::core::convert::From<InvalidHeader> for BetaVaultErrors {
        fn from(value: InvalidHeader) -> Self {
            Self::InvalidHeader(value)
        }
    }
    impl ::core::convert::From<InvalidMerkleBranch> for BetaVaultErrors {
        fn from(value: InvalidMerkleBranch) -> Self {
            Self::InvalidMerkleBranch(value)
        }
    }
    impl ::core::convert::From<InvalidParams> for BetaVaultErrors {
        fn from(value: InvalidParams) -> Self {
            Self::InvalidParams(value)
        }
    }
    impl ::core::convert::From<KindMismatch> for BetaVaultErrors {
        fn from(value: KindMismatch) -> Self {
            Self::KindMismatch(value)
        }
    }
    impl ::core::convert::From<MalformedStatement> for BetaVaultErrors {
        fn from(value: MalformedStatement) -> Self {
            Self::MalformedStatement(value)
        }
    }
    impl ::core::convert::From<MalformedTx> for BetaVaultErrors {
        fn from(value: MalformedTx) -> Self {
            Self::MalformedTx(value)
        }
    }
    impl ::core::convert::From<NoParty> for BetaVaultErrors {
        fn from(value: NoParty) -> Self {
            Self::NoParty(value)
        }
    }
    impl ::core::convert::From<NotApproved> for BetaVaultErrors {
        fn from(value: NotApproved) -> Self {
            Self::NotApproved(value)
        }
    }
    impl ::core::convert::From<NotAuditor> for BetaVaultErrors {
        fn from(value: NotAuditor) -> Self {
            Self::NotAuditor(value)
        }
    }
    impl ::core::convert::From<NotOnStatementChain> for BetaVaultErrors {
        fn from(value: NotOnStatementChain) -> Self {
            Self::NotOnStatementChain(value)
        }
    }
    impl ::core::convert::From<NotOperator> for BetaVaultErrors {
        fn from(value: NotOperator) -> Self {
            Self::NotOperator(value)
        }
    }
    impl ::core::convert::From<NotReady> for BetaVaultErrors {
        fn from(value: NotReady) -> Self {
            Self::NotReady(value)
        }
    }
    impl ::core::convert::From<PartyDead> for BetaVaultErrors {
        fn from(value: PartyDead) -> Self {
            Self::PartyDead(value)
        }
    }
    impl ::core::convert::From<PartyExists> for BetaVaultErrors {
        fn from(value: PartyExists) -> Self {
            Self::PartyExists(value)
        }
    }
    impl ::core::convert::From<Paused> for BetaVaultErrors {
        fn from(value: Paused) -> Self {
            Self::Paused(value)
        }
    }
    impl ::core::convert::From<RateLimited> for BetaVaultErrors {
        fn from(value: RateLimited) -> Self {
            Self::RateLimited(value)
        }
    }
    impl ::core::convert::From<ReentrancyGuardReentrantCall> for BetaVaultErrors {
        fn from(value: ReentrancyGuardReentrantCall) -> Self {
            Self::ReentrancyGuardReentrantCall(value)
        }
    }
    impl ::core::convert::From<RefundNotReady> for BetaVaultErrors {
        fn from(value: RefundNotReady) -> Self {
            Self::RefundNotReady(value)
        }
    }
    impl ::core::convert::From<ReleaseTokenUnsupported> for BetaVaultErrors {
        fn from(value: ReleaseTokenUnsupported) -> Self {
            Self::ReleaseTokenUnsupported(value)
        }
    }
    impl ::core::convert::From<SafeERC20FailedOperation> for BetaVaultErrors {
        fn from(value: SafeERC20FailedOperation) -> Self {
            Self::SafeERC20FailedOperation(value)
        }
    }
    impl ::core::convert::From<SkipNotReady> for BetaVaultErrors {
        fn from(value: SkipNotReady) -> Self {
            Self::SkipNotReady(value)
        }
    }
    impl ::core::convert::From<StatementHashMismatch> for BetaVaultErrors {
        fn from(value: StatementHashMismatch) -> Self {
            Self::StatementHashMismatch(value)
        }
    }
    impl ::core::convert::From<TargetNotProcessed> for BetaVaultErrors {
        fn from(value: TargetNotProcessed) -> Self {
            Self::TargetNotProcessed(value)
        }
    }
    impl ::core::convert::From<TokenNotRegistered> for BetaVaultErrors {
        fn from(value: TokenNotRegistered) -> Self {
            Self::TokenNotRegistered(value)
        }
    }
    impl ::core::convert::From<TransferFailed> for BetaVaultErrors {
        fn from(value: TransferFailed) -> Self {
            Self::TransferFailed(value)
        }
    }
    impl ::core::convert::From<TxidMismatch> for BetaVaultErrors {
        fn from(value: TxidMismatch) -> Self {
            Self::TxidMismatch(value)
        }
    }
    impl ::core::convert::From<Unauthorized> for BetaVaultErrors {
        fn from(value: Unauthorized) -> Self {
            Self::Unauthorized(value)
        }
    }
    impl ::core::convert::From<UnbondNotReady> for BetaVaultErrors {
        fn from(value: UnbondNotReady) -> Self {
            Self::UnbondNotReady(value)
        }
    }
    impl ::core::convert::From<UnbondNotRequested> for BetaVaultErrors {
        fn from(value: UnbondNotRequested) -> Self {
            Self::UnbondNotRequested(value)
        }
    }
    impl ::core::convert::From<WitnessSerialization> for BetaVaultErrors {
        fn from(value: WitnessSerialization) -> Self {
            Self::WitnessSerialization(value)
        }
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(
        name = "AnchorProcessed",
        abi = "AnchorProcessed(bytes32,bytes32,uint8,uint8)"
    )]
    pub struct AnchorProcessedFilter {
        #[ethevent(indexed)]
        pub party_id: [u8; 32],
        #[ethevent(indexed)]
        pub txid_le: [u8; 32],
        pub kind: u8,
        pub status: u8,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(
        name = "Deposited",
        abi = "Deposited(uint64,bytes32,uint64,uint64,uint64,address,address)"
    )]
    pub struct DepositedFilter {
        #[ethevent(indexed)]
        pub lock_id: u64,
        #[ethevent(indexed)]
        pub sol_user: [u8; 32],
        pub nonce: u64,
        pub units: u64,
        pub deadline: u64,
        pub depositor: ::ethers::core::types::Address,
        pub token: ::ethers::core::types::Address,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(name = "Finalized", abi = "Finalized(uint64,bytes32)")]
    pub struct FinalizedFilter {
        #[ethevent(indexed)]
        pub lock_id: u64,
        pub anchor_txid_le: [u8; 32],
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(
        name = "PartyRegistered",
        abi = "PartyRegistered(bytes32,uint8,address,uint256)"
    )]
    pub struct PartyRegisteredFilter {
        #[ethevent(indexed)]
        pub party_id: [u8; 32],
        pub kind: u8,
        pub owner: ::ethers::core::types::Address,
        pub bond: ::ethers::core::types::U256,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(name = "Refunded", abi = "Refunded(uint64)")]
    pub struct RefundedFilter {
        #[ethevent(indexed)]
        pub lock_id: u64,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(name = "ReleaseHeld", abi = "ReleaseHeld(bytes32,bool)")]
    pub struct ReleaseHeldFilter {
        #[ethevent(indexed)]
        pub txid_le: [u8; 32],
        pub held: bool,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(
        name = "ReleasePaidFromEscrow",
        abi = "ReleasePaidFromEscrow(bytes32,bytes32,address,uint256)"
    )]
    pub struct ReleasePaidFromEscrowFilter {
        #[ethevent(indexed)]
        pub txid_le: [u8; 32],
        #[ethevent(indexed)]
        pub attester: [u8; 32],
        pub to: ::ethers::core::types::Address,
        pub wei: ::ethers::core::types::U256,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(
        name = "ReleaseQueued",
        abi = "ReleaseQueued(bytes32,uint64,address,uint64,uint64)"
    )]
    pub struct ReleaseQueuedFilter {
        #[ethevent(indexed)]
        pub txid_le: [u8; 32],
        #[ethevent(indexed)]
        pub lock_id: u64,
        pub to: ::ethers::core::types::Address,
        pub units: u64,
        pub release_after: u64,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(name = "ReleaseSettled", abi = "ReleaseSettled(bytes32,bool)")]
    pub struct ReleaseSettledFilter {
        #[ethevent(indexed)]
        pub txid_le: [u8; 32],
        pub reimbursed: bool,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(name = "Released", abi = "Released(bytes32,uint64,address,uint256)")]
    pub struct ReleasedFilter {
        #[ethevent(indexed)]
        pub txid_le: [u8; 32],
        #[ethevent(indexed)]
        pub lock_id: u64,
        pub to: ::ethers::core::types::Address,
        pub wei: ::ethers::core::types::U256,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(name = "Slashed", abi = "Slashed(bytes32,uint256,address)")]
    pub struct SlashedFilter {
        #[ethevent(indexed)]
        pub party_id: [u8; 32],
        pub amount: ::ethers::core::types::U256,
        pub submitter: ::ethers::core::types::Address,
    }
    #[derive(
        Clone,
        ::ethers::contract::EthEvent,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethevent(name = "TokenParamsSet", abi = "TokenParamsSet(address,uint256,uint256)")]
    pub struct TokenParamsSetFilter {
        #[ethevent(indexed)]
        pub token: ::ethers::core::types::Address,
        pub amount_per_unit: ::ethers::core::types::U256,
        pub slash_wei_per_unit: ::ethers::core::types::U256,
    }
    ///Container type for all of the contract's events
    #[derive(Clone, ::ethers::contract::EthAbiType, Debug, PartialEq, Eq, Hash)]
    pub enum BetaVaultEvents {
        AnchorProcessedFilter(AnchorProcessedFilter),
        DepositedFilter(DepositedFilter),
        FinalizedFilter(FinalizedFilter),
        PartyRegisteredFilter(PartyRegisteredFilter),
        RefundedFilter(RefundedFilter),
        ReleaseHeldFilter(ReleaseHeldFilter),
        ReleasePaidFromEscrowFilter(ReleasePaidFromEscrowFilter),
        ReleaseQueuedFilter(ReleaseQueuedFilter),
        ReleaseSettledFilter(ReleaseSettledFilter),
        ReleasedFilter(ReleasedFilter),
        SlashedFilter(SlashedFilter),
        TokenParamsSetFilter(TokenParamsSetFilter),
    }
    impl ::ethers::contract::EthLogDecode for BetaVaultEvents {
        fn decode_log(
            log: &::ethers::core::abi::RawLog,
        ) -> ::core::result::Result<Self, ::ethers::core::abi::Error> {
            if let Ok(decoded) = AnchorProcessedFilter::decode_log(log) {
                return Ok(BetaVaultEvents::AnchorProcessedFilter(decoded));
            }
            if let Ok(decoded) = DepositedFilter::decode_log(log) {
                return Ok(BetaVaultEvents::DepositedFilter(decoded));
            }
            if let Ok(decoded) = FinalizedFilter::decode_log(log) {
                return Ok(BetaVaultEvents::FinalizedFilter(decoded));
            }
            if let Ok(decoded) = PartyRegisteredFilter::decode_log(log) {
                return Ok(BetaVaultEvents::PartyRegisteredFilter(decoded));
            }
            if let Ok(decoded) = RefundedFilter::decode_log(log) {
                return Ok(BetaVaultEvents::RefundedFilter(decoded));
            }
            if let Ok(decoded) = ReleaseHeldFilter::decode_log(log) {
                return Ok(BetaVaultEvents::ReleaseHeldFilter(decoded));
            }
            if let Ok(decoded) = ReleasePaidFromEscrowFilter::decode_log(log) {
                return Ok(BetaVaultEvents::ReleasePaidFromEscrowFilter(decoded));
            }
            if let Ok(decoded) = ReleaseQueuedFilter::decode_log(log) {
                return Ok(BetaVaultEvents::ReleaseQueuedFilter(decoded));
            }
            if let Ok(decoded) = ReleaseSettledFilter::decode_log(log) {
                return Ok(BetaVaultEvents::ReleaseSettledFilter(decoded));
            }
            if let Ok(decoded) = ReleasedFilter::decode_log(log) {
                return Ok(BetaVaultEvents::ReleasedFilter(decoded));
            }
            if let Ok(decoded) = SlashedFilter::decode_log(log) {
                return Ok(BetaVaultEvents::SlashedFilter(decoded));
            }
            if let Ok(decoded) = TokenParamsSetFilter::decode_log(log) {
                return Ok(BetaVaultEvents::TokenParamsSetFilter(decoded));
            }
            Err(::ethers::core::abi::Error::InvalidData)
        }
    }
    impl ::core::fmt::Display for BetaVaultEvents {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            match self {
                Self::AnchorProcessedFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::DepositedFilter(element) => ::core::fmt::Display::fmt(element, f),
                Self::FinalizedFilter(element) => ::core::fmt::Display::fmt(element, f),
                Self::PartyRegisteredFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::RefundedFilter(element) => ::core::fmt::Display::fmt(element, f),
                Self::ReleaseHeldFilter(element) => ::core::fmt::Display::fmt(element, f),
                Self::ReleasePaidFromEscrowFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::ReleaseQueuedFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::ReleaseSettledFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::ReleasedFilter(element) => ::core::fmt::Display::fmt(element, f),
                Self::SlashedFilter(element) => ::core::fmt::Display::fmt(element, f),
                Self::TokenParamsSetFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
            }
        }
    }
    impl ::core::convert::From<AnchorProcessedFilter> for BetaVaultEvents {
        fn from(value: AnchorProcessedFilter) -> Self {
            Self::AnchorProcessedFilter(value)
        }
    }
    impl ::core::convert::From<DepositedFilter> for BetaVaultEvents {
        fn from(value: DepositedFilter) -> Self {
            Self::DepositedFilter(value)
        }
    }
    impl ::core::convert::From<FinalizedFilter> for BetaVaultEvents {
        fn from(value: FinalizedFilter) -> Self {
            Self::FinalizedFilter(value)
        }
    }
    impl ::core::convert::From<PartyRegisteredFilter> for BetaVaultEvents {
        fn from(value: PartyRegisteredFilter) -> Self {
            Self::PartyRegisteredFilter(value)
        }
    }
    impl ::core::convert::From<RefundedFilter> for BetaVaultEvents {
        fn from(value: RefundedFilter) -> Self {
            Self::RefundedFilter(value)
        }
    }
    impl ::core::convert::From<ReleaseHeldFilter> for BetaVaultEvents {
        fn from(value: ReleaseHeldFilter) -> Self {
            Self::ReleaseHeldFilter(value)
        }
    }
    impl ::core::convert::From<ReleasePaidFromEscrowFilter> for BetaVaultEvents {
        fn from(value: ReleasePaidFromEscrowFilter) -> Self {
            Self::ReleasePaidFromEscrowFilter(value)
        }
    }
    impl ::core::convert::From<ReleaseQueuedFilter> for BetaVaultEvents {
        fn from(value: ReleaseQueuedFilter) -> Self {
            Self::ReleaseQueuedFilter(value)
        }
    }
    impl ::core::convert::From<ReleaseSettledFilter> for BetaVaultEvents {
        fn from(value: ReleaseSettledFilter) -> Self {
            Self::ReleaseSettledFilter(value)
        }
    }
    impl ::core::convert::From<ReleasedFilter> for BetaVaultEvents {
        fn from(value: ReleasedFilter) -> Self {
            Self::ReleasedFilter(value)
        }
    }
    impl ::core::convert::From<SlashedFilter> for BetaVaultEvents {
        fn from(value: SlashedFilter) -> Self {
            Self::SlashedFilter(value)
        }
    }
    impl ::core::convert::From<TokenParamsSetFilter> for BetaVaultEvents {
        fn from(value: TokenParamsSetFilter) -> Self {
            Self::TokenParamsSetFilter(value)
        }
    }
    ///Container type for all input parameters for the `ANCHOR_VERSION` function with signature `ANCHOR_VERSION()` and selector `0x2d998d18`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "ANCHOR_VERSION", abi = "ANCHOR_VERSION()")]
    pub struct AnchorVersionCall;
    ///Container type for all input parameters for the `BPS_DENOM` function with signature `BPS_DENOM()` and selector `0x6637e38c`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "BPS_DENOM", abi = "BPS_DENOM()")]
    pub struct BpsDenomCall;
    ///Container type for all input parameters for the `KIND_ALIVE` function with signature `KIND_ALIVE()` and selector `0x8649ff4c`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "KIND_ALIVE", abi = "KIND_ALIVE()")]
    pub struct KindAliveCall;
    ///Container type for all input parameters for the `KIND_ATTEST` function with signature `KIND_ATTEST()` and selector `0x388043f4`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "KIND_ATTEST", abi = "KIND_ATTEST()")]
    pub struct KindAttestCall;
    ///Container type for all input parameters for the `KIND_CANCEL` function with signature `KIND_CANCEL()` and selector `0xc4b8d3f9`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "KIND_CANCEL", abi = "KIND_CANCEL()")]
    pub struct KindCancelCall;
    ///Container type for all input parameters for the `KIND_CLEAR` function with signature `KIND_CLEAR()` and selector `0x232b88c1`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "KIND_CLEAR", abi = "KIND_CLEAR()")]
    pub struct KindClearCall;
    ///Container type for all input parameters for the `KIND_MINT` function with signature `KIND_MINT()` and selector `0x81d2f871`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "KIND_MINT", abi = "KIND_MINT()")]
    pub struct KindMintCall;
    ///Container type for all input parameters for the `KIND_RELEASE` function with signature `KIND_RELEASE()` and selector `0xeab1f2c6`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "KIND_RELEASE", abi = "KIND_RELEASE()")]
    pub struct KindReleaseCall;
    ///Container type for all input parameters for the `KIND_VETO` function with signature `KIND_VETO()` and selector `0xfb7e66b3`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "KIND_VETO", abi = "KIND_VETO()")]
    pub struct KindVetoCall;
    ///Container type for all input parameters for the `anchors` function with signature `anchors(bytes32)` and selector `0xb01b6d53`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "anchors", abi = "anchors(bytes32)")]
    pub struct AnchorsCall(pub [u8; 32]);
    ///Container type for all input parameters for the `approveOperator` function with signature `approveOperator(bytes32,address)` and selector `0xfebbaac2`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "approveOperator", abi = "approveOperator(bytes32,address)")]
    pub struct ApproveOperatorCall {
        pub party_id: [u8; 32],
        pub owner: ::ethers::core::types::Address,
    }
    ///Container type for all input parameters for the `approvedOperators` function with signature `approvedOperators(bytes32)` and selector `0x677dd834`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "approvedOperators", abi = "approvedOperators(bytes32)")]
    pub struct ApprovedOperatorsCall(pub [u8; 32]);
    ///Container type for all input parameters for the `deposit` function with signature `deposit(address,bytes32,uint64,uint64,uint64)` and selector `0xe039676a`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "deposit", abi = "deposit(address,bytes32,uint64,uint64,uint64)")]
    pub struct DepositCall {
        pub token: ::ethers::core::types::Address,
        pub sol_user: [u8; 32],
        pub nonce: u64,
        pub units: u64,
        pub deadline: u64,
    }
    ///Container type for all input parameters for the `executeRelease` function with signature `executeRelease(bytes32)` and selector `0xc799147b`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "executeRelease", abi = "executeRelease(bytes32)")]
    pub struct ExecuteReleaseCall {
        pub txid_le: [u8; 32],
    }
    ///Container type for all input parameters for the `fundRewards` function with signature `fundRewards()` and selector `0xff18bf0b`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "fundRewards", abi = "fundRewards()")]
    pub struct FundRewardsCall;
    ///Container type for all input parameters for the `governance` function with signature `governance()` and selector `0x5aa6e675`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "governance", abi = "governance()")]
    pub struct GovernanceCall;
    ///Container type for all input parameters for the `insurance` function with signature `insurance()` and selector `0x89cf3204`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "insurance", abi = "insurance()")]
    pub struct InsuranceCall;
    ///Container type for all input parameters for the `insuranceReserved` function with signature `insuranceReserved()` and selector `0x28cfdafe`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "insuranceReserved", abi = "insuranceReserved()")]
    pub struct InsuranceReservedCall;
    ///Container type for all input parameters for the `ipowHeaders` function with signature `ipowHeaders()` and selector `0x21d5dc2f`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "ipowHeaders", abi = "ipowHeaders()")]
    pub struct IpowHeadersCall;
    ///Container type for all input parameters for the `locks` function with signature `locks(uint64)` and selector `0x26043c0e`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "locks", abi = "locks(uint64)")]
    pub struct LocksCall(pub u64);
    ///Container type for all input parameters for the `mintAttester` function with signature `mintAttester(bytes32)` and selector `0x0f55b6bf`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "mintAttester", abi = "mintAttester(bytes32)")]
    pub struct MintAttesterCall(pub [u8; 32]);
    ///Container type for all input parameters for the `nextLockId` function with signature `nextLockId()` and selector `0x6518a0b3`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "nextLockId", abi = "nextLockId()")]
    pub struct NextLockIdCall;
    ///Container type for all input parameters for the `params` function with signature `params()` and selector `0xcff0ab96`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "params", abi = "params()")]
    pub struct ParamsCall;
    ///Container type for all input parameters for the `parties` function with signature `parties(bytes32)` and selector `0x941a581c`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "parties", abi = "parties(bytes32)")]
    pub struct PartiesCall(pub [u8; 32]);
    ///Container type for all input parameters for the `paused` function with signature `paused()` and selector `0x5c975abb`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "paused", abi = "paused()")]
    pub struct PausedCall;
    ///Container type for all input parameters for the `processAnchor` function with signature `processAnchor(bytes32,bytes,bytes,uint256,bytes32[],uint256)` and selector `0xc7635b80`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(
        name = "processAnchor",
        abi = "processAnchor(bytes32,bytes,bytes,uint256,bytes32[],uint256)"
    )]
    pub struct ProcessAnchorCall {
        pub party_id: [u8; 32],
        pub statement: ::ethers::core::types::Bytes,
        pub tx_raw: ::ethers::core::types::Bytes,
        pub block_height: ::ethers::core::types::U256,
        pub branch_le: ::std::vec::Vec<[u8; 32]>,
        pub index: ::ethers::core::types::U256,
    }
    ///Container type for all input parameters for the `refund` function with signature `refund(uint64)` and selector `0xd7194ccb`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "refund", abi = "refund(uint64)")]
    pub struct RefundCall {
        pub lock_id: u64,
    }
    ///Container type for all input parameters for the `registerParty` function with signature `registerParty(bytes32,uint8,bytes32,uint32)` and selector `0x58772a1e`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(
        name = "registerParty",
        abi = "registerParty(bytes32,uint8,bytes32,uint32)"
    )]
    pub struct RegisterPartyCall {
        pub party_id: [u8; 32],
        pub kind: u8,
        pub anchor_txid_le: [u8; 32],
        pub anchor_vout: u32,
    }
    ///Container type for all input parameters for the `requestUnbond` function with signature `requestUnbond(bytes32)` and selector `0x977d178f`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "requestUnbond", abi = "requestUnbond(bytes32)")]
    pub struct RequestUnbondCall {
        pub party_id: [u8; 32],
    }
    ///Container type for all input parameters for the `rewardPool` function with signature `rewardPool()` and selector `0x66666aa9`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "rewardPool", abi = "rewardPool()")]
    pub struct RewardPoolCall;
    ///Container type for all input parameters for the `setParams` function with signature `setParams((uint256,uint64,uint64,uint64,uint64,uint64,uint256,uint256,uint256,uint256,uint16),bool)` and selector `0xa9998a5f`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(
        name = "setParams",
        abi = "setParams((uint256,uint64,uint64,uint64,uint64,uint64,uint256,uint256,uint256,uint256,uint16),bool)"
    )]
    pub struct SetParamsCall {
        pub p: Params,
        pub paused: bool,
    }
    ///Container type for all input parameters for the `setTokenParams` function with signature `setTokenParams(address,(uint256,uint256))` and selector `0x4fd6eca7`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(
        name = "setTokenParams",
        abi = "setTokenParams(address,(uint256,uint256))"
    )]
    pub struct SetTokenParamsCall {
        pub token: ::ethers::core::types::Address,
        pub p: TokenParams,
    }
    ///Container type for all input parameters for the `settleRelease` function with signature `settleRelease(bytes32)` and selector `0x2acb293b`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "settleRelease", abi = "settleRelease(bytes32)")]
    pub struct SettleReleaseCall {
        pub txid_le: [u8; 32],
    }
    ///Container type for all input parameters for the `skipAnchor` function with signature `skipAnchor(bytes32,bytes,uint256,bytes32[],uint256)` and selector `0xad1d0e8e`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(
        name = "skipAnchor",
        abi = "skipAnchor(bytes32,bytes,uint256,bytes32[],uint256)"
    )]
    pub struct SkipAnchorCall {
        pub party_id: [u8; 32],
        pub tx_raw: ::ethers::core::types::Bytes,
        pub block_height: ::ethers::core::types::U256,
        pub branch_le: ::std::vec::Vec<[u8; 32]>,
        pub index: ::ethers::core::types::U256,
    }
    ///Container type for all input parameters for the `tokenParams` function with signature `tokenParams(address)` and selector `0x012374c9`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "tokenParams", abi = "tokenParams(address)")]
    pub struct TokenParamsCall(pub ::ethers::core::types::Address);
    ///Container type for all input parameters for the `topUpBond` function with signature `topUpBond(bytes32)` and selector `0x1cb8fed9`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "topUpBond", abi = "topUpBond(bytes32)")]
    pub struct TopUpBondCall {
        pub party_id: [u8; 32],
    }
    ///Container type for all input parameters for the `totalBonds` function with signature `totalBonds()` and selector `0xf263c470`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "totalBonds", abi = "totalBonds()")]
    pub struct TotalBondsCall;
    ///Container type for all input parameters for the `totalLocked` function with signature `totalLocked(address)` and selector `0xd8fb9337`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "totalLocked", abi = "totalLocked(address)")]
    pub struct TotalLockedCall(pub ::ethers::core::types::Address);
    ///Container type for all input parameters for the `withdrawBond` function with signature `withdrawBond(bytes32)` and selector `0x7285e1ac`
    #[derive(
        Clone,
        ::ethers::contract::EthCall,
        ::ethers::contract::EthDisplay,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    #[ethcall(name = "withdrawBond", abi = "withdrawBond(bytes32)")]
    pub struct WithdrawBondCall {
        pub party_id: [u8; 32],
    }
    ///Container type for all of the contract's call
    #[derive(Clone, ::ethers::contract::EthAbiType, Debug, PartialEq, Eq, Hash)]
    pub enum BetaVaultCalls {
        AnchorVersion(AnchorVersionCall),
        BpsDenom(BpsDenomCall),
        KindAlive(KindAliveCall),
        KindAttest(KindAttestCall),
        KindCancel(KindCancelCall),
        KindClear(KindClearCall),
        KindMint(KindMintCall),
        KindRelease(KindReleaseCall),
        KindVeto(KindVetoCall),
        Anchors(AnchorsCall),
        ApproveOperator(ApproveOperatorCall),
        ApprovedOperators(ApprovedOperatorsCall),
        Deposit(DepositCall),
        ExecuteRelease(ExecuteReleaseCall),
        FundRewards(FundRewardsCall),
        Governance(GovernanceCall),
        Insurance(InsuranceCall),
        InsuranceReserved(InsuranceReservedCall),
        IpowHeaders(IpowHeadersCall),
        Locks(LocksCall),
        MintAttester(MintAttesterCall),
        NextLockId(NextLockIdCall),
        Params(ParamsCall),
        Parties(PartiesCall),
        Paused(PausedCall),
        ProcessAnchor(ProcessAnchorCall),
        Refund(RefundCall),
        RegisterParty(RegisterPartyCall),
        RequestUnbond(RequestUnbondCall),
        RewardPool(RewardPoolCall),
        SetParams(SetParamsCall),
        SetTokenParams(SetTokenParamsCall),
        SettleRelease(SettleReleaseCall),
        SkipAnchor(SkipAnchorCall),
        TokenParams(TokenParamsCall),
        TopUpBond(TopUpBondCall),
        TotalBonds(TotalBondsCall),
        TotalLocked(TotalLockedCall),
        WithdrawBond(WithdrawBondCall),
    }
    impl ::ethers::core::abi::AbiDecode for BetaVaultCalls {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::core::result::Result<Self, ::ethers::core::abi::AbiError> {
            let data = data.as_ref();
            if let Ok(decoded) = <AnchorVersionCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::AnchorVersion(decoded));
            }
            if let Ok(decoded) = <BpsDenomCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::BpsDenom(decoded));
            }
            if let Ok(decoded) = <KindAliveCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::KindAlive(decoded));
            }
            if let Ok(decoded) = <KindAttestCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::KindAttest(decoded));
            }
            if let Ok(decoded) = <KindCancelCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::KindCancel(decoded));
            }
            if let Ok(decoded) = <KindClearCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::KindClear(decoded));
            }
            if let Ok(decoded) = <KindMintCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::KindMint(decoded));
            }
            if let Ok(decoded) = <KindReleaseCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::KindRelease(decoded));
            }
            if let Ok(decoded) = <KindVetoCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::KindVeto(decoded));
            }
            if let Ok(decoded) = <AnchorsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Anchors(decoded));
            }
            if let Ok(decoded) = <ApproveOperatorCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ApproveOperator(decoded));
            }
            if let Ok(decoded) = <ApprovedOperatorsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ApprovedOperators(decoded));
            }
            if let Ok(decoded) = <DepositCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Deposit(decoded));
            }
            if let Ok(decoded) = <ExecuteReleaseCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ExecuteRelease(decoded));
            }
            if let Ok(decoded) = <FundRewardsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::FundRewards(decoded));
            }
            if let Ok(decoded) = <GovernanceCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Governance(decoded));
            }
            if let Ok(decoded) = <InsuranceCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Insurance(decoded));
            }
            if let Ok(decoded) = <InsuranceReservedCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::InsuranceReserved(decoded));
            }
            if let Ok(decoded) = <IpowHeadersCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::IpowHeaders(decoded));
            }
            if let Ok(decoded) = <LocksCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Locks(decoded));
            }
            if let Ok(decoded) = <MintAttesterCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::MintAttester(decoded));
            }
            if let Ok(decoded) = <NextLockIdCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NextLockId(decoded));
            }
            if let Ok(decoded) = <ParamsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Params(decoded));
            }
            if let Ok(decoded) = <PartiesCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Parties(decoded));
            }
            if let Ok(decoded) = <PausedCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Paused(decoded));
            }
            if let Ok(decoded) = <ProcessAnchorCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ProcessAnchor(decoded));
            }
            if let Ok(decoded) = <RefundCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Refund(decoded));
            }
            if let Ok(decoded) = <RegisterPartyCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RegisterParty(decoded));
            }
            if let Ok(decoded) = <RequestUnbondCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RequestUnbond(decoded));
            }
            if let Ok(decoded) = <RewardPoolCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RewardPool(decoded));
            }
            if let Ok(decoded) = <SetParamsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SetParams(decoded));
            }
            if let Ok(decoded) = <SetTokenParamsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SetTokenParams(decoded));
            }
            if let Ok(decoded) = <SettleReleaseCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SettleRelease(decoded));
            }
            if let Ok(decoded) = <SkipAnchorCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SkipAnchor(decoded));
            }
            if let Ok(decoded) = <TokenParamsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TokenParams(decoded));
            }
            if let Ok(decoded) = <TopUpBondCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TopUpBond(decoded));
            }
            if let Ok(decoded) = <TotalBondsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TotalBonds(decoded));
            }
            if let Ok(decoded) = <TotalLockedCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TotalLocked(decoded));
            }
            if let Ok(decoded) = <WithdrawBondCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::WithdrawBond(decoded));
            }
            Err(::ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ::ethers::core::abi::AbiEncode for BetaVaultCalls {
        fn encode(self) -> Vec<u8> {
            match self {
                Self::AnchorVersion(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::BpsDenom(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::KindAlive(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::KindAttest(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::KindCancel(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::KindClear(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::KindMint(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::KindRelease(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::KindVeto(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Anchors(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::ApproveOperator(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ApprovedOperators(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Deposit(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::ExecuteRelease(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::FundRewards(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Governance(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Insurance(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::InsuranceReserved(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::IpowHeaders(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Locks(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::MintAttester(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::NextLockId(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Params(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::Parties(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::Paused(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::ProcessAnchor(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Refund(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::RegisterParty(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::RequestUnbond(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::RewardPool(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SetParams(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SetTokenParams(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SettleRelease(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SkipAnchor(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TokenParams(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TopUpBond(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TotalBonds(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TotalLocked(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::WithdrawBond(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
            }
        }
    }
    impl ::core::fmt::Display for BetaVaultCalls {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            match self {
                Self::AnchorVersion(element) => ::core::fmt::Display::fmt(element, f),
                Self::BpsDenom(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindAlive(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindAttest(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindCancel(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindClear(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindMint(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindRelease(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindVeto(element) => ::core::fmt::Display::fmt(element, f),
                Self::Anchors(element) => ::core::fmt::Display::fmt(element, f),
                Self::ApproveOperator(element) => ::core::fmt::Display::fmt(element, f),
                Self::ApprovedOperators(element) => ::core::fmt::Display::fmt(element, f),
                Self::Deposit(element) => ::core::fmt::Display::fmt(element, f),
                Self::ExecuteRelease(element) => ::core::fmt::Display::fmt(element, f),
                Self::FundRewards(element) => ::core::fmt::Display::fmt(element, f),
                Self::Governance(element) => ::core::fmt::Display::fmt(element, f),
                Self::Insurance(element) => ::core::fmt::Display::fmt(element, f),
                Self::InsuranceReserved(element) => ::core::fmt::Display::fmt(element, f),
                Self::IpowHeaders(element) => ::core::fmt::Display::fmt(element, f),
                Self::Locks(element) => ::core::fmt::Display::fmt(element, f),
                Self::MintAttester(element) => ::core::fmt::Display::fmt(element, f),
                Self::NextLockId(element) => ::core::fmt::Display::fmt(element, f),
                Self::Params(element) => ::core::fmt::Display::fmt(element, f),
                Self::Parties(element) => ::core::fmt::Display::fmt(element, f),
                Self::Paused(element) => ::core::fmt::Display::fmt(element, f),
                Self::ProcessAnchor(element) => ::core::fmt::Display::fmt(element, f),
                Self::Refund(element) => ::core::fmt::Display::fmt(element, f),
                Self::RegisterParty(element) => ::core::fmt::Display::fmt(element, f),
                Self::RequestUnbond(element) => ::core::fmt::Display::fmt(element, f),
                Self::RewardPool(element) => ::core::fmt::Display::fmt(element, f),
                Self::SetParams(element) => ::core::fmt::Display::fmt(element, f),
                Self::SetTokenParams(element) => ::core::fmt::Display::fmt(element, f),
                Self::SettleRelease(element) => ::core::fmt::Display::fmt(element, f),
                Self::SkipAnchor(element) => ::core::fmt::Display::fmt(element, f),
                Self::TokenParams(element) => ::core::fmt::Display::fmt(element, f),
                Self::TopUpBond(element) => ::core::fmt::Display::fmt(element, f),
                Self::TotalBonds(element) => ::core::fmt::Display::fmt(element, f),
                Self::TotalLocked(element) => ::core::fmt::Display::fmt(element, f),
                Self::WithdrawBond(element) => ::core::fmt::Display::fmt(element, f),
            }
        }
    }
    impl ::core::convert::From<AnchorVersionCall> for BetaVaultCalls {
        fn from(value: AnchorVersionCall) -> Self {
            Self::AnchorVersion(value)
        }
    }
    impl ::core::convert::From<BpsDenomCall> for BetaVaultCalls {
        fn from(value: BpsDenomCall) -> Self {
            Self::BpsDenom(value)
        }
    }
    impl ::core::convert::From<KindAliveCall> for BetaVaultCalls {
        fn from(value: KindAliveCall) -> Self {
            Self::KindAlive(value)
        }
    }
    impl ::core::convert::From<KindAttestCall> for BetaVaultCalls {
        fn from(value: KindAttestCall) -> Self {
            Self::KindAttest(value)
        }
    }
    impl ::core::convert::From<KindCancelCall> for BetaVaultCalls {
        fn from(value: KindCancelCall) -> Self {
            Self::KindCancel(value)
        }
    }
    impl ::core::convert::From<KindClearCall> for BetaVaultCalls {
        fn from(value: KindClearCall) -> Self {
            Self::KindClear(value)
        }
    }
    impl ::core::convert::From<KindMintCall> for BetaVaultCalls {
        fn from(value: KindMintCall) -> Self {
            Self::KindMint(value)
        }
    }
    impl ::core::convert::From<KindReleaseCall> for BetaVaultCalls {
        fn from(value: KindReleaseCall) -> Self {
            Self::KindRelease(value)
        }
    }
    impl ::core::convert::From<KindVetoCall> for BetaVaultCalls {
        fn from(value: KindVetoCall) -> Self {
            Self::KindVeto(value)
        }
    }
    impl ::core::convert::From<AnchorsCall> for BetaVaultCalls {
        fn from(value: AnchorsCall) -> Self {
            Self::Anchors(value)
        }
    }
    impl ::core::convert::From<ApproveOperatorCall> for BetaVaultCalls {
        fn from(value: ApproveOperatorCall) -> Self {
            Self::ApproveOperator(value)
        }
    }
    impl ::core::convert::From<ApprovedOperatorsCall> for BetaVaultCalls {
        fn from(value: ApprovedOperatorsCall) -> Self {
            Self::ApprovedOperators(value)
        }
    }
    impl ::core::convert::From<DepositCall> for BetaVaultCalls {
        fn from(value: DepositCall) -> Self {
            Self::Deposit(value)
        }
    }
    impl ::core::convert::From<ExecuteReleaseCall> for BetaVaultCalls {
        fn from(value: ExecuteReleaseCall) -> Self {
            Self::ExecuteRelease(value)
        }
    }
    impl ::core::convert::From<FundRewardsCall> for BetaVaultCalls {
        fn from(value: FundRewardsCall) -> Self {
            Self::FundRewards(value)
        }
    }
    impl ::core::convert::From<GovernanceCall> for BetaVaultCalls {
        fn from(value: GovernanceCall) -> Self {
            Self::Governance(value)
        }
    }
    impl ::core::convert::From<InsuranceCall> for BetaVaultCalls {
        fn from(value: InsuranceCall) -> Self {
            Self::Insurance(value)
        }
    }
    impl ::core::convert::From<InsuranceReservedCall> for BetaVaultCalls {
        fn from(value: InsuranceReservedCall) -> Self {
            Self::InsuranceReserved(value)
        }
    }
    impl ::core::convert::From<IpowHeadersCall> for BetaVaultCalls {
        fn from(value: IpowHeadersCall) -> Self {
            Self::IpowHeaders(value)
        }
    }
    impl ::core::convert::From<LocksCall> for BetaVaultCalls {
        fn from(value: LocksCall) -> Self {
            Self::Locks(value)
        }
    }
    impl ::core::convert::From<MintAttesterCall> for BetaVaultCalls {
        fn from(value: MintAttesterCall) -> Self {
            Self::MintAttester(value)
        }
    }
    impl ::core::convert::From<NextLockIdCall> for BetaVaultCalls {
        fn from(value: NextLockIdCall) -> Self {
            Self::NextLockId(value)
        }
    }
    impl ::core::convert::From<ParamsCall> for BetaVaultCalls {
        fn from(value: ParamsCall) -> Self {
            Self::Params(value)
        }
    }
    impl ::core::convert::From<PartiesCall> for BetaVaultCalls {
        fn from(value: PartiesCall) -> Self {
            Self::Parties(value)
        }
    }
    impl ::core::convert::From<PausedCall> for BetaVaultCalls {
        fn from(value: PausedCall) -> Self {
            Self::Paused(value)
        }
    }
    impl ::core::convert::From<ProcessAnchorCall> for BetaVaultCalls {
        fn from(value: ProcessAnchorCall) -> Self {
            Self::ProcessAnchor(value)
        }
    }
    impl ::core::convert::From<RefundCall> for BetaVaultCalls {
        fn from(value: RefundCall) -> Self {
            Self::Refund(value)
        }
    }
    impl ::core::convert::From<RegisterPartyCall> for BetaVaultCalls {
        fn from(value: RegisterPartyCall) -> Self {
            Self::RegisterParty(value)
        }
    }
    impl ::core::convert::From<RequestUnbondCall> for BetaVaultCalls {
        fn from(value: RequestUnbondCall) -> Self {
            Self::RequestUnbond(value)
        }
    }
    impl ::core::convert::From<RewardPoolCall> for BetaVaultCalls {
        fn from(value: RewardPoolCall) -> Self {
            Self::RewardPool(value)
        }
    }
    impl ::core::convert::From<SetParamsCall> for BetaVaultCalls {
        fn from(value: SetParamsCall) -> Self {
            Self::SetParams(value)
        }
    }
    impl ::core::convert::From<SetTokenParamsCall> for BetaVaultCalls {
        fn from(value: SetTokenParamsCall) -> Self {
            Self::SetTokenParams(value)
        }
    }
    impl ::core::convert::From<SettleReleaseCall> for BetaVaultCalls {
        fn from(value: SettleReleaseCall) -> Self {
            Self::SettleRelease(value)
        }
    }
    impl ::core::convert::From<SkipAnchorCall> for BetaVaultCalls {
        fn from(value: SkipAnchorCall) -> Self {
            Self::SkipAnchor(value)
        }
    }
    impl ::core::convert::From<TokenParamsCall> for BetaVaultCalls {
        fn from(value: TokenParamsCall) -> Self {
            Self::TokenParams(value)
        }
    }
    impl ::core::convert::From<TopUpBondCall> for BetaVaultCalls {
        fn from(value: TopUpBondCall) -> Self {
            Self::TopUpBond(value)
        }
    }
    impl ::core::convert::From<TotalBondsCall> for BetaVaultCalls {
        fn from(value: TotalBondsCall) -> Self {
            Self::TotalBonds(value)
        }
    }
    impl ::core::convert::From<TotalLockedCall> for BetaVaultCalls {
        fn from(value: TotalLockedCall) -> Self {
            Self::TotalLocked(value)
        }
    }
    impl ::core::convert::From<WithdrawBondCall> for BetaVaultCalls {
        fn from(value: WithdrawBondCall) -> Self {
            Self::WithdrawBond(value)
        }
    }
    ///Container type for all return fields from the `ANCHOR_VERSION` function with signature `ANCHOR_VERSION()` and selector `0x2d998d18`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct AnchorVersionReturn(pub u8);
    ///Container type for all return fields from the `BPS_DENOM` function with signature `BPS_DENOM()` and selector `0x6637e38c`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct BpsDenomReturn(pub ::ethers::core::types::U256);
    ///Container type for all return fields from the `KIND_ALIVE` function with signature `KIND_ALIVE()` and selector `0x8649ff4c`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct KindAliveReturn(pub u8);
    ///Container type for all return fields from the `KIND_ATTEST` function with signature `KIND_ATTEST()` and selector `0x388043f4`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct KindAttestReturn(pub u8);
    ///Container type for all return fields from the `KIND_CANCEL` function with signature `KIND_CANCEL()` and selector `0xc4b8d3f9`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct KindCancelReturn(pub u8);
    ///Container type for all return fields from the `KIND_CLEAR` function with signature `KIND_CLEAR()` and selector `0x232b88c1`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct KindClearReturn(pub u8);
    ///Container type for all return fields from the `KIND_MINT` function with signature `KIND_MINT()` and selector `0x81d2f871`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct KindMintReturn(pub u8);
    ///Container type for all return fields from the `KIND_RELEASE` function with signature `KIND_RELEASE()` and selector `0xeab1f2c6`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct KindReleaseReturn(pub u8);
    ///Container type for all return fields from the `KIND_VETO` function with signature `KIND_VETO()` and selector `0xfb7e66b3`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct KindVetoReturn(pub u8);
    ///Container type for all return fields from the `anchors` function with signature `anchors(bytes32)` and selector `0xb01b6d53`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct AnchorsReturn {
        pub party_id: [u8; 32],
        pub kind: u8,
        pub status: u8,
        pub statement_hash: [u8; 32],
        pub block_height: u64,
        pub processed_at: u64,
        pub lock_id: u64,
        pub to: ::ethers::core::types::Address,
        pub units: u64,
        pub challenge_until: u64,
        pub held: bool,
        pub attester: [u8; 32],
        pub escrow: ::ethers::core::types::U256,
        pub paid: bool,
        pub settled: bool,
    }
    ///Container type for all return fields from the `approvedOperators` function with signature `approvedOperators(bytes32)` and selector `0x677dd834`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct ApprovedOperatorsReturn(pub ::ethers::core::types::Address);
    ///Container type for all return fields from the `deposit` function with signature `deposit(address,bytes32,uint64,uint64,uint64)` and selector `0xe039676a`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct DepositReturn {
        pub lock_id: u64,
    }
    ///Container type for all return fields from the `governance` function with signature `governance()` and selector `0x5aa6e675`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct GovernanceReturn(pub ::ethers::core::types::Address);
    ///Container type for all return fields from the `insurance` function with signature `insurance()` and selector `0x89cf3204`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct InsuranceReturn(pub ::ethers::core::types::U256);
    ///Container type for all return fields from the `insuranceReserved` function with signature `insuranceReserved()` and selector `0x28cfdafe`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct InsuranceReservedReturn(pub ::ethers::core::types::U256);
    ///Container type for all return fields from the `ipowHeaders` function with signature `ipowHeaders()` and selector `0x21d5dc2f`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct IpowHeadersReturn(pub ::ethers::core::types::Address);
    ///Container type for all return fields from the `locks` function with signature `locks(uint64)` and selector `0x26043c0e`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct LocksReturn {
        pub sol_user: [u8; 32],
        pub nonce: u64,
        pub units: u64,
        pub deadline: u64,
        pub state: u8,
        pub depositor: ::ethers::core::types::Address,
        pub token: ::ethers::core::types::Address,
        pub amount: ::ethers::core::types::U256,
    }
    ///Container type for all return fields from the `mintAttester` function with signature `mintAttester(bytes32)` and selector `0x0f55b6bf`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct MintAttesterReturn(pub [u8; 32]);
    ///Container type for all return fields from the `nextLockId` function with signature `nextLockId()` and selector `0x6518a0b3`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct NextLockIdReturn(pub u64);
    ///Container type for all return fields from the `params` function with signature `params()` and selector `0xcff0ab96`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct ParamsReturn {
        pub eth_wei_per_unit: ::ethers::core::types::U256,
        pub t_fin_secs: u64,
        pub t_challenge_secs: u64,
        pub t_skip_secs: u64,
        pub refund_margin_secs: u64,
        pub unbond_delay_secs: u64,
        pub min_operator_bond: ::ethers::core::types::U256,
        pub min_auditor_bond: ::ethers::core::types::U256,
        pub veto_slash_wei: ::ethers::core::types::U256,
        pub veto_reward_wei: ::ethers::core::types::U256,
        pub bounty_bps: u16,
    }
    ///Container type for all return fields from the `parties` function with signature `parties(bytes32)` and selector `0x941a581c`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct PartiesReturn {
        pub exists: bool,
        pub owner: ::ethers::core::types::Address,
        pub kind: u8,
        pub anchor_txid_le: [u8; 32],
        pub anchor_vout: u32,
        pub seq: u64,
        pub bond: ::ethers::core::types::U256,
        pub dead: bool,
        pub unbond_requested_at: u64,
    }
    ///Container type for all return fields from the `paused` function with signature `paused()` and selector `0x5c975abb`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct PausedReturn(pub bool);
    ///Container type for all return fields from the `rewardPool` function with signature `rewardPool()` and selector `0x66666aa9`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct RewardPoolReturn(pub ::ethers::core::types::U256);
    ///Container type for all return fields from the `tokenParams` function with signature `tokenParams(address)` and selector `0x012374c9`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct TokenParamsReturn {
        pub amount_per_unit: ::ethers::core::types::U256,
        pub slash_wei_per_unit: ::ethers::core::types::U256,
    }
    ///Container type for all return fields from the `totalBonds` function with signature `totalBonds()` and selector `0xf263c470`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct TotalBondsReturn(pub ::ethers::core::types::U256);
    ///Container type for all return fields from the `totalLocked` function with signature `totalLocked(address)` and selector `0xd8fb9337`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct TotalLockedReturn(pub ::ethers::core::types::U256);
    ///`Params(uint256,uint64,uint64,uint64,uint64,uint64,uint256,uint256,uint256,uint256,uint16)`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct Params {
        pub eth_wei_per_unit: ::ethers::core::types::U256,
        pub t_fin_secs: u64,
        pub t_challenge_secs: u64,
        pub t_skip_secs: u64,
        pub refund_margin_secs: u64,
        pub unbond_delay_secs: u64,
        pub min_operator_bond: ::ethers::core::types::U256,
        pub min_auditor_bond: ::ethers::core::types::U256,
        pub veto_slash_wei: ::ethers::core::types::U256,
        pub veto_reward_wei: ::ethers::core::types::U256,
        pub bounty_bps: u16,
    }
    ///`TokenParams(uint256,uint256)`
    #[derive(
        Clone,
        ::ethers::contract::EthAbiType,
        ::ethers::contract::EthAbiCodec,
        Default,
        Debug,
        PartialEq,
        Eq,
        Hash
    )]
    pub struct TokenParams {
        pub amount_per_unit: ::ethers::core::types::U256,
        pub slash_wei_per_unit: ::ethers::core::types::U256,
    }
}

