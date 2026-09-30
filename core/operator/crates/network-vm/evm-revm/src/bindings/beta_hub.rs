// @generated — DO NOT EDIT
#![allow(clippy::module_inception)]

pub use beta_hub::*;
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
pub mod beta_hub {
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
                        name: ::std::borrow::ToOwned::to_owned("_ipowHeadersAddr"),
                        kind: ::ethers::core::abi::ethabi::ParamType::Address,
                        internal_type: ::core::option::Option::Some(
                            ::std::borrow::ToOwned::to_owned("address"),
                        ),
                    },
                    ::ethers::core::abi::ethabi::Param {
                        name: ::std::borrow::ToOwned::to_owned("_selfNetworkId"),
                        kind: ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                        internal_type: ::core::option::Option::Some(
                            ::std::borrow::ToOwned::to_owned("uint256"),
                        ),
                    },
                    ::ethers::core::abi::ethabi::Param {
                        name: ::std::borrow::ToOwned::to_owned("_params"),
                        kind: ::ethers::core::abi::ethabi::ParamType::Tuple(
                            ::std::vec![
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                ::ethers::core::abi::ethabi::ParamType::Uint(16usize),
                            ],
                        ),
                        internal_type: ::core::option::Option::Some(
                            ::std::borrow::ToOwned::to_owned("struct BetaHub.Params"),
                        ),
                    },
                    ::ethers::core::abi::ethabi::Param {
                        name: ::std::borrow::ToOwned::to_owned("tokenName"),
                        kind: ::ethers::core::abi::ethabi::ParamType::String,
                        internal_type: ::core::option::Option::Some(
                            ::std::borrow::ToOwned::to_owned("string"),
                        ),
                    },
                    ::ethers::core::abi::ethabi::Param {
                        name: ::std::borrow::ToOwned::to_owned("tokenSymbol"),
                        kind: ::ethers::core::abi::ethabi::ParamType::String,
                        internal_type: ::core::option::Option::Some(
                            ::std::borrow::ToOwned::to_owned("string"),
                        ),
                    },
                ],
            }),
            functions: ::core::convert::From::from([
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
                    ::std::borrow::ToOwned::to_owned("MAX_COMPONENTS"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("MAX_COMPONENTS"),
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
                    ::std::borrow::ToOwned::to_owned("MAX_LOCAL_COMPONENTS"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned(
                                "MAX_LOCAL_COMPONENTS",
                            ),
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
                    ::std::borrow::ToOwned::to_owned("SELF_NETWORK_ID"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("SELF_NETWORK_ID"),
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
                    ::std::borrow::ToOwned::to_owned("SOLANA_NETWORK_ID"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("SOLANA_NETWORK_ID"),
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
                                            "enum HubAnchorJudge.AnchorStatus",
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
                                    name: ::std::borrow::ToOwned::to_owned("compositionId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("componentIndex"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(8usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint8"),
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
                                    name: ::std::borrow::ToOwned::to_owned("settled"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("pendingId"),
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
                    ::std::borrow::ToOwned::to_owned("approvePending"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("approvePending"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("nonce"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("remoteLockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Array(
                                        ::std::boxed::Box::new(
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                        ),
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64[]"),
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
                    ::std::borrow::ToOwned::to_owned("compositionExists"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("compositionExists"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("id"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
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
                    ::std::borrow::ToOwned::to_owned("exerciseMint"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("exerciseMint"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("user"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("nonce"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("minted"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(
                                        256usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint256"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::NonPayable,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("expirePending"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("expirePending"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("user"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("nonce"),
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
                    ::std::borrow::ToOwned::to_owned("getComposition"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("getComposition"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("id"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                            ],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Array(
                                        ::std::boxed::Box::new(
                                            ::ethers::core::abi::ethabi::ParamType::Tuple(
                                                ::std::vec![
                                                    ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                                    ::ethers::core::abi::ethabi::ParamType::Address,
                                                    ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                                ],
                                            ),
                                        ),
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned(
                                            "struct HubCompositionRegistry.Component[]",
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
                    ::std::borrow::ToOwned::to_owned("lockLocal"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("lockLocal"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("compositionId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
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
                                    name: ::std::borrow::ToOwned::to_owned("pendingId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::Payable,
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
                    ::std::borrow::ToOwned::to_owned("params"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("params"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
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
                                    name: ::std::borrow::ToOwned::to_owned("slashWeiPerUnit"),
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
                                        ::std::borrow::ToOwned::to_owned(
                                            "enum HubPartyRegistry.PartyKind",
                                        ),
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
                    ::std::borrow::ToOwned::to_owned("pending"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("pending"),
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
                                    name: ::std::borrow::ToOwned::to_owned("user"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("address"),
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
                                    name: ::std::borrow::ToOwned::to_owned("compositionId"),
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
                                    name: ::std::borrow::ToOwned::to_owned("approved"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Bool,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bool"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("queuedBy"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("createdAt"),
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
                    ::std::borrow::ToOwned::to_owned("pendingRemoteAnchorTxid"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned(
                                "pendingRemoteAnchorTxid",
                            ),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("pendingId"),
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
                                    kind: ::ethers::core::abi::ethabi::ParamType::Array(
                                        ::std::boxed::Box::new(
                                            ::ethers::core::abi::ethabi::ParamType::FixedBytes(32usize),
                                        ),
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("bytes32[]"),
                                    ),
                                },
                            ],
                            constant: ::core::option::Option::None,
                            state_mutability: ::ethers::core::abi::ethabi::StateMutability::View,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("pendingRemoteLockId"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned(
                                "pendingRemoteLockId",
                            ),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("pendingId"),
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
                                    kind: ::ethers::core::abi::ethabi::ParamType::Array(
                                        ::std::boxed::Box::new(
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                        ),
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64[]"),
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
                    ::std::borrow::ToOwned::to_owned("registerComposition"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned(
                                "registerComposition",
                            ),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("id"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("uint64"),
                                    ),
                                },
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::borrow::ToOwned::to_owned("components"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Array(
                                        ::std::boxed::Box::new(
                                            ::ethers::core::abi::ethabi::ParamType::Tuple(
                                                ::std::vec![
                                                    ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                                    ::ethers::core::abi::ethabi::ParamType::Address,
                                                    ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                                ],
                                            ),
                                        ),
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned(
                                            "struct HubCompositionRegistry.Component[]",
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
                                        ::std::borrow::ToOwned::to_owned(
                                            "enum HubPartyRegistry.PartyKind",
                                        ),
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
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(256usize),
                                            ::ethers::core::abi::ethabi::ParamType::Uint(16usize),
                                        ],
                                    ),
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("struct BetaHub.Params"),
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
                    ::std::borrow::ToOwned::to_owned("settleMint"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("settleMint"),
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
                    ::std::borrow::ToOwned::to_owned("token"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Function {
                            name: ::std::borrow::ToOwned::to_owned("token"),
                            inputs: ::std::vec![],
                            outputs: ::std::vec![
                                ::ethers::core::abi::ethabi::Param {
                                    name: ::std::string::String::new(),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    internal_type: ::core::option::Option::Some(
                                        ::std::borrow::ToOwned::to_owned("contract HubToken"),
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
                    ::std::borrow::ToOwned::to_owned("ComponentHeld"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("ComponentHeld"),
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
                    ::std::borrow::ToOwned::to_owned("CompositionRegistered"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned(
                                "CompositionRegistered",
                            ),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("id"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("componentCount"),
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
                    ::std::borrow::ToOwned::to_owned("Minted"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("Minted"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("pendingId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("user"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("amount"),
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
                    ::std::borrow::ToOwned::to_owned("PendingApproved"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("PendingApproved"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("pendingId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("remoteLockId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Array(
                                        ::std::boxed::Box::new(
                                            ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                        ),
                                    ),
                                    indexed: false,
                                },
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("PendingCreated"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("PendingCreated"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("pendingId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("user"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
                                    indexed: true,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("nonce"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Uint(64usize),
                                    indexed: false,
                                },
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("compositionId"),
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
                            ],
                            anonymous: false,
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("PendingExpired"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::Event {
                            name: ::std::borrow::ToOwned::to_owned("PendingExpired"),
                            inputs: ::std::vec![
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("pendingId"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::FixedBytes(
                                        32usize,
                                    ),
                                    indexed: true,
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
                                ::ethers::core::abi::ethabi::EventParam {
                                    name: ::std::borrow::ToOwned::to_owned("remainderTo"),
                                    kind: ::ethers::core::abi::ethabi::ParamType::Address,
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
                    ::std::borrow::ToOwned::to_owned("AlreadyApproved"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("AlreadyApproved"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
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
                    ::std::borrow::ToOwned::to_owned("BondTooSmall"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("BondTooSmall"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("ComponentHeldOrNotReady"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "ComponentHeldOrNotReady",
                            ),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("CompositionExists"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("CompositionExists"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("DuplicateComponent"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("DuplicateComponent"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("IncompleteRemoteComponents"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "IncompleteRemoteComponents",
                            ),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("InvalidComponents"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("InvalidComponents"),
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
                    ::std::borrow::ToOwned::to_owned("NoComposition"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NoComposition"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("NoLocalComponent"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NoLocalComponent"),
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
                    ::std::borrow::ToOwned::to_owned("NoPending"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NoPending"),
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
                    ::std::borrow::ToOwned::to_owned("NotPendingUser"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("NotPendingUser"),
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
                    ::std::borrow::ToOwned::to_owned("PendingExists"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("PendingExists"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("QueuedByLiveOperator"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "QueuedByLiveOperator",
                            ),
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
                    ::std::borrow::ToOwned::to_owned("RemoteCountMismatch"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned(
                                "RemoteCountMismatch",
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
                    ::std::borrow::ToOwned::to_owned("TooManyLocal"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("TooManyLocal"),
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
                    ::std::borrow::ToOwned::to_owned("UnsupportedKind"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("UnsupportedKind"),
                            inputs: ::std::vec![],
                        },
                    ],
                ),
                (
                    ::std::borrow::ToOwned::to_owned("UnsupportedNetwork"),
                    ::std::vec![
                        ::ethers::core::abi::ethabi::AbiError {
                            name: ::std::borrow::ToOwned::to_owned("UnsupportedNetwork"),
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
            receive: false,
            fallback: false,
        }
    }
    ///The parsed JSON ABI of the contract.
    pub static BETAHUB_ABI: ::ethers::contract::Lazy<::ethers::core::abi::Abi> = ::ethers::contract::Lazy::new(
        __abi,
    );
    pub struct BetaHub<M>(::ethers::contract::Contract<M>);
    impl<M> ::core::clone::Clone for BetaHub<M> {
        fn clone(&self) -> Self {
            Self(::core::clone::Clone::clone(&self.0))
        }
    }
    impl<M> ::core::ops::Deref for BetaHub<M> {
        type Target = ::ethers::contract::Contract<M>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<M> ::core::ops::DerefMut for BetaHub<M> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl<M> ::core::fmt::Debug for BetaHub<M> {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple(::core::stringify!(BetaHub)).field(&self.address()).finish()
        }
    }
    impl<M: ::ethers::providers::Middleware> BetaHub<M> {
        /// Creates a new contract instance with the specified `ethers` client at
        /// `address`. The contract derefs to a `ethers::Contract` object.
        pub fn new<T: Into<::ethers::core::types::Address>>(
            address: T,
            client: ::std::sync::Arc<M>,
        ) -> Self {
            Self(
                ::ethers::contract::Contract::new(
                    address.into(),
                    BETAHUB_ABI.clone(),
                    client,
                ),
            )
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
        ///Calls the contract's `MAX_COMPONENTS` (0x0ab50a6a) function
        pub fn max_components(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([10, 181, 10, 106], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `MAX_LOCAL_COMPONENTS` (0x01548fd5) function
        pub fn max_local_components(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([1, 84, 143, 213], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `SELF_NETWORK_ID` (0xede4754a) function
        pub fn self_network_id(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([237, 228, 117, 74], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `SOLANA_NETWORK_ID` (0xf0494276) function
        pub fn solana_network_id(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([240, 73, 66, 118], ())
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
                u8,
                u64,
                u64,
                u64,
                bool,
                bool,
                [u8; 32],
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
        ///Calls the contract's `approvePending` (0x6799cdb5) function
        pub fn approve_pending(
            &self,
            nonce: u64,
            remote_lock_id: ::std::vec::Vec<u64>,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([103, 153, 205, 181], (nonce, remote_lock_id))
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
        ///Calls the contract's `compositionExists` (0x36f88572) function
        pub fn composition_exists(
            &self,
            id: u64,
        ) -> ::ethers::contract::builders::ContractCall<M, bool> {
            self.0
                .method_hash([54, 248, 133, 114], id)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `exerciseMint` (0x9b0b90ff) function
        pub fn exercise_mint(
            &self,
            user: ::ethers::core::types::Address,
            nonce: u64,
        ) -> ::ethers::contract::builders::ContractCall<M, ::ethers::core::types::U256> {
            self.0
                .method_hash([155, 11, 144, 255], (user, nonce))
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `expirePending` (0x015622bc) function
        pub fn expire_pending(
            &self,
            user: ::ethers::core::types::Address,
            nonce: u64,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([1, 86, 34, 188], (user, nonce))
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `fundRewards` (0xff18bf0b) function
        pub fn fund_rewards(&self) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([255, 24, 191, 11], ())
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `getComposition` (0xb2fd6cab) function
        pub fn get_composition(
            &self,
            id: u64,
        ) -> ::ethers::contract::builders::ContractCall<M, ::std::vec::Vec<Component>> {
            self.0
                .method_hash([178, 253, 108, 171], id)
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
        ///Calls the contract's `lockLocal` (0x40ec1309) function
        pub fn lock_local(
            &self,
            composition_id: u64,
            nonce: u64,
            units: u64,
            deadline: u64,
        ) -> ::ethers::contract::builders::ContractCall<M, [u8; 32]> {
            self.0
                .method_hash([64, 236, 19, 9], (composition_id, nonce, units, deadline))
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
        ///Calls the contract's `params` (0xcff0ab96) function
        pub fn params(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            (
                u64,
                u64,
                u64,
                u64,
                ::ethers::core::types::U256,
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
        ///Calls the contract's `pending` (0x1808eeb8) function
        pub fn pending(
            &self,
            p0: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            (::ethers::core::types::Address, u64, u64, u64, u64, bool, [u8; 32], u64),
        > {
            self.0
                .method_hash([24, 8, 238, 184], p0)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `pendingRemoteAnchorTxid` (0x2b72faff) function
        pub fn pending_remote_anchor_txid(
            &self,
            pending_id: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, ::std::vec::Vec<[u8; 32]>> {
            self.0
                .method_hash([43, 114, 250, 255], pending_id)
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `pendingRemoteLockId` (0xa02dafef) function
        pub fn pending_remote_lock_id(
            &self,
            pending_id: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, ::std::vec::Vec<u64>> {
            self.0
                .method_hash([160, 45, 175, 239], pending_id)
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
        ///Calls the contract's `registerComposition` (0xfeebd416) function
        pub fn register_composition(
            &self,
            id: u64,
            components: ::std::vec::Vec<Component>,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([254, 235, 212, 22], (id, components))
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
        ///Calls the contract's `setParams` (0x09ebc914) function
        pub fn set_params(
            &self,
            p: Params,
            paused: bool,
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([9, 235, 201, 20], (p, paused))
                .expect("method not found (this should never happen)")
        }
        ///Calls the contract's `settleMint` (0x91d3a9c0) function
        pub fn settle_mint(
            &self,
            txid_le: [u8; 32],
        ) -> ::ethers::contract::builders::ContractCall<M, ()> {
            self.0
                .method_hash([145, 211, 169, 192], txid_le)
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
        ///Calls the contract's `token` (0xfc0c546a) function
        pub fn token(
            &self,
        ) -> ::ethers::contract::builders::ContractCall<
            M,
            ::ethers::core::types::Address,
        > {
            self.0
                .method_hash([252, 12, 84, 106], ())
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
        ///Gets the contract's `ComponentHeld` event
        pub fn component_held_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            ComponentHeldFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `CompositionRegistered` event
        pub fn composition_registered_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            CompositionRegisteredFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `Minted` event
        pub fn minted_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<::std::sync::Arc<M>, M, MintedFilter> {
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
        ///Gets the contract's `PendingApproved` event
        pub fn pending_approved_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            PendingApprovedFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `PendingCreated` event
        pub fn pending_created_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            PendingCreatedFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `PendingExpired` event
        pub fn pending_expired_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<
            ::std::sync::Arc<M>,
            M,
            PendingExpiredFilter,
        > {
            self.0.event()
        }
        ///Gets the contract's `Slashed` event
        pub fn slashed_filter(
            &self,
        ) -> ::ethers::contract::builders::Event<::std::sync::Arc<M>, M, SlashedFilter> {
            self.0.event()
        }
        /// Returns an `Event` builder for all the events of this contract.
        pub fn events(
            &self,
        ) -> ::ethers::contract::builders::Event<::std::sync::Arc<M>, M, BetaHubEvents> {
            self.0.event_with_filter(::core::default::Default::default())
        }
    }
    impl<M: ::ethers::providers::Middleware> From<::ethers::contract::Contract<M>>
    for BetaHub<M> {
        fn from(contract: ::ethers::contract::Contract<M>) -> Self {
            Self::new(contract.address(), contract.client())
        }
    }
    ///Custom Error type `AlreadyApproved` with signature `AlreadyApproved()` and selector `0x101f817a`
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
    #[etherror(name = "AlreadyApproved", abi = "AlreadyApproved()")]
    pub struct AlreadyApproved;
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
    ///Custom Error type `ComponentHeldOrNotReady` with signature `ComponentHeldOrNotReady()` and selector `0xe066429a`
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
    #[etherror(name = "ComponentHeldOrNotReady", abi = "ComponentHeldOrNotReady()")]
    pub struct ComponentHeldOrNotReady;
    ///Custom Error type `CompositionExists` with signature `CompositionExists()` and selector `0xdf20677d`
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
    #[etherror(name = "CompositionExists", abi = "CompositionExists()")]
    pub struct CompositionExists;
    ///Custom Error type `DuplicateComponent` with signature `DuplicateComponent()` and selector `0x2e5ba0aa`
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
    #[etherror(name = "DuplicateComponent", abi = "DuplicateComponent()")]
    pub struct DuplicateComponent;
    ///Custom Error type `IncompleteRemoteComponents` with signature `IncompleteRemoteComponents()` and selector `0xb31b2acf`
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
        name = "IncompleteRemoteComponents",
        abi = "IncompleteRemoteComponents()"
    )]
    pub struct IncompleteRemoteComponents;
    ///Custom Error type `InvalidComponents` with signature `InvalidComponents()` and selector `0xf04ced8b`
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
    #[etherror(name = "InvalidComponents", abi = "InvalidComponents()")]
    pub struct InvalidComponents;
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
    ///Custom Error type `NoComposition` with signature `NoComposition()` and selector `0x64b7d9c4`
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
    #[etherror(name = "NoComposition", abi = "NoComposition()")]
    pub struct NoComposition;
    ///Custom Error type `NoLocalComponent` with signature `NoLocalComponent()` and selector `0x39c3448f`
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
    #[etherror(name = "NoLocalComponent", abi = "NoLocalComponent()")]
    pub struct NoLocalComponent;
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
    ///Custom Error type `NoPending` with signature `NoPending()` and selector `0xda7557bc`
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
    #[etherror(name = "NoPending", abi = "NoPending()")]
    pub struct NoPending;
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
    ///Custom Error type `NotPendingUser` with signature `NotPendingUser()` and selector `0x13638c9c`
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
    #[etherror(name = "NotPendingUser", abi = "NotPendingUser()")]
    pub struct NotPendingUser;
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
    ///Custom Error type `PendingExists` with signature `PendingExists()` and selector `0x4b1a898d`
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
    #[etherror(name = "PendingExists", abi = "PendingExists()")]
    pub struct PendingExists;
    ///Custom Error type `QueuedByLiveOperator` with signature `QueuedByLiveOperator()` and selector `0xd188c916`
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
    #[etherror(name = "QueuedByLiveOperator", abi = "QueuedByLiveOperator()")]
    pub struct QueuedByLiveOperator;
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
    ///Custom Error type `RemoteCountMismatch` with signature `RemoteCountMismatch()` and selector `0xe740dea8`
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
    #[etherror(name = "RemoteCountMismatch", abi = "RemoteCountMismatch()")]
    pub struct RemoteCountMismatch;
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
    ///Custom Error type `TooManyLocal` with signature `TooManyLocal()` and selector `0xa52d8994`
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
    #[etherror(name = "TooManyLocal", abi = "TooManyLocal()")]
    pub struct TooManyLocal;
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
    ///Custom Error type `UnsupportedKind` with signature `UnsupportedKind()` and selector `0x67e16e7f`
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
    #[etherror(name = "UnsupportedKind", abi = "UnsupportedKind()")]
    pub struct UnsupportedKind;
    ///Custom Error type `UnsupportedNetwork` with signature `UnsupportedNetwork()` and selector `0x6e6544ae`
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
    #[etherror(name = "UnsupportedNetwork", abi = "UnsupportedNetwork()")]
    pub struct UnsupportedNetwork;
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
    pub enum BetaHubErrors {
        AlreadyApproved(AlreadyApproved),
        AlreadyProcessed(AlreadyProcessed),
        BadAnchorPayload(BadAnchorPayload),
        BadAnchorState(BadAnchorState),
        BondTooSmall(BondTooSmall),
        ComponentHeldOrNotReady(ComponentHeldOrNotReady),
        CompositionExists(CompositionExists),
        DuplicateComponent(DuplicateComponent),
        IncompleteRemoteComponents(IncompleteRemoteComponents),
        InvalidComponents(InvalidComponents),
        InvalidHeader(InvalidHeader),
        InvalidMerkleBranch(InvalidMerkleBranch),
        InvalidParams(InvalidParams),
        KindMismatch(KindMismatch),
        MalformedStatement(MalformedStatement),
        MalformedTx(MalformedTx),
        NoComposition(NoComposition),
        NoLocalComponent(NoLocalComponent),
        NoParty(NoParty),
        NoPending(NoPending),
        NotApproved(NotApproved),
        NotAuditor(NotAuditor),
        NotOnStatementChain(NotOnStatementChain),
        NotOperator(NotOperator),
        NotPendingUser(NotPendingUser),
        PartyDead(PartyDead),
        PartyExists(PartyExists),
        Paused(Paused),
        PendingExists(PendingExists),
        QueuedByLiveOperator(QueuedByLiveOperator),
        ReentrancyGuardReentrantCall(ReentrancyGuardReentrantCall),
        RefundNotReady(RefundNotReady),
        RemoteCountMismatch(RemoteCountMismatch),
        SafeERC20FailedOperation(SafeERC20FailedOperation),
        SkipNotReady(SkipNotReady),
        StatementHashMismatch(StatementHashMismatch),
        TargetNotProcessed(TargetNotProcessed),
        TooManyLocal(TooManyLocal),
        TransferFailed(TransferFailed),
        Unauthorized(Unauthorized),
        UnbondNotReady(UnbondNotReady),
        UnbondNotRequested(UnbondNotRequested),
        UnsupportedKind(UnsupportedKind),
        UnsupportedNetwork(UnsupportedNetwork),
        WitnessSerialization(WitnessSerialization),
        /// The standard solidity revert string, with selector
        /// Error(string) -- 0x08c379a0
        RevertString(::std::string::String),
    }
    impl ::ethers::core::abi::AbiDecode for BetaHubErrors {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::core::result::Result<Self, ::ethers::core::abi::AbiError> {
            let data = data.as_ref();
            if let Ok(decoded) = <::std::string::String as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RevertString(decoded));
            }
            if let Ok(decoded) = <AlreadyApproved as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::AlreadyApproved(decoded));
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
            if let Ok(decoded) = <BondTooSmall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::BondTooSmall(decoded));
            }
            if let Ok(decoded) = <ComponentHeldOrNotReady as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ComponentHeldOrNotReady(decoded));
            }
            if let Ok(decoded) = <CompositionExists as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::CompositionExists(decoded));
            }
            if let Ok(decoded) = <DuplicateComponent as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::DuplicateComponent(decoded));
            }
            if let Ok(decoded) = <IncompleteRemoteComponents as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::IncompleteRemoteComponents(decoded));
            }
            if let Ok(decoded) = <InvalidComponents as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::InvalidComponents(decoded));
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
            if let Ok(decoded) = <NoComposition as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NoComposition(decoded));
            }
            if let Ok(decoded) = <NoLocalComponent as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NoLocalComponent(decoded));
            }
            if let Ok(decoded) = <NoParty as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NoParty(decoded));
            }
            if let Ok(decoded) = <NoPending as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NoPending(decoded));
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
            if let Ok(decoded) = <NotPendingUser as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::NotPendingUser(decoded));
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
            if let Ok(decoded) = <PendingExists as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::PendingExists(decoded));
            }
            if let Ok(decoded) = <QueuedByLiveOperator as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::QueuedByLiveOperator(decoded));
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
            if let Ok(decoded) = <RemoteCountMismatch as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RemoteCountMismatch(decoded));
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
            if let Ok(decoded) = <TooManyLocal as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TooManyLocal(decoded));
            }
            if let Ok(decoded) = <TransferFailed as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::TransferFailed(decoded));
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
            if let Ok(decoded) = <UnsupportedKind as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::UnsupportedKind(decoded));
            }
            if let Ok(decoded) = <UnsupportedNetwork as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::UnsupportedNetwork(decoded));
            }
            if let Ok(decoded) = <WitnessSerialization as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::WitnessSerialization(decoded));
            }
            Err(::ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ::ethers::core::abi::AbiEncode for BetaHubErrors {
        fn encode(self) -> ::std::vec::Vec<u8> {
            match self {
                Self::AlreadyApproved(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::AlreadyProcessed(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::BadAnchorPayload(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::BadAnchorState(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::BondTooSmall(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ComponentHeldOrNotReady(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::CompositionExists(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::DuplicateComponent(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::IncompleteRemoteComponents(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::InvalidComponents(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
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
                Self::NoComposition(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::NoLocalComponent(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::NoParty(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::NoPending(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
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
                Self::NotPendingUser(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::PartyDead(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::PartyExists(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Paused(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::PendingExists(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::QueuedByLiveOperator(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ReentrancyGuardReentrantCall(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::RefundNotReady(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::RemoteCountMismatch(element) => {
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
                Self::TooManyLocal(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TransferFailed(element) => {
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
                Self::UnsupportedKind(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::UnsupportedNetwork(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::WitnessSerialization(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::RevertString(s) => ::ethers::core::abi::AbiEncode::encode(s),
            }
        }
    }
    impl ::ethers::contract::ContractRevert for BetaHubErrors {
        fn valid_selector(selector: [u8; 4]) -> bool {
            match selector {
                [0x08, 0xc3, 0x79, 0xa0] => true,
                _ if selector
                    == <AlreadyApproved as ::ethers::contract::EthError>::selector() => {
                    true
                }
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
                    == <BondTooSmall as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <ComponentHeldOrNotReady as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <CompositionExists as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <DuplicateComponent as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <IncompleteRemoteComponents as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <InvalidComponents as ::ethers::contract::EthError>::selector() => {
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
                    == <NoComposition as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <NoLocalComponent as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <NoParty as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <NoPending as ::ethers::contract::EthError>::selector() => true,
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
                    == <NotPendingUser as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <PartyDead as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <PartyExists as ::ethers::contract::EthError>::selector() => true,
                _ if selector == <Paused as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <PendingExists as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <QueuedByLiveOperator as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <ReentrancyGuardReentrantCall as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <RefundNotReady as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <RemoteCountMismatch as ::ethers::contract::EthError>::selector() => {
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
                    == <TooManyLocal as ::ethers::contract::EthError>::selector() => true,
                _ if selector
                    == <TransferFailed as ::ethers::contract::EthError>::selector() => {
                    true
                }
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
                    == <UnsupportedKind as ::ethers::contract::EthError>::selector() => {
                    true
                }
                _ if selector
                    == <UnsupportedNetwork as ::ethers::contract::EthError>::selector() => {
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
    impl ::core::fmt::Display for BetaHubErrors {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            match self {
                Self::AlreadyApproved(element) => ::core::fmt::Display::fmt(element, f),
                Self::AlreadyProcessed(element) => ::core::fmt::Display::fmt(element, f),
                Self::BadAnchorPayload(element) => ::core::fmt::Display::fmt(element, f),
                Self::BadAnchorState(element) => ::core::fmt::Display::fmt(element, f),
                Self::BondTooSmall(element) => ::core::fmt::Display::fmt(element, f),
                Self::ComponentHeldOrNotReady(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::CompositionExists(element) => ::core::fmt::Display::fmt(element, f),
                Self::DuplicateComponent(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::IncompleteRemoteComponents(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::InvalidComponents(element) => ::core::fmt::Display::fmt(element, f),
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
                Self::NoComposition(element) => ::core::fmt::Display::fmt(element, f),
                Self::NoLocalComponent(element) => ::core::fmt::Display::fmt(element, f),
                Self::NoParty(element) => ::core::fmt::Display::fmt(element, f),
                Self::NoPending(element) => ::core::fmt::Display::fmt(element, f),
                Self::NotApproved(element) => ::core::fmt::Display::fmt(element, f),
                Self::NotAuditor(element) => ::core::fmt::Display::fmt(element, f),
                Self::NotOnStatementChain(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::NotOperator(element) => ::core::fmt::Display::fmt(element, f),
                Self::NotPendingUser(element) => ::core::fmt::Display::fmt(element, f),
                Self::PartyDead(element) => ::core::fmt::Display::fmt(element, f),
                Self::PartyExists(element) => ::core::fmt::Display::fmt(element, f),
                Self::Paused(element) => ::core::fmt::Display::fmt(element, f),
                Self::PendingExists(element) => ::core::fmt::Display::fmt(element, f),
                Self::QueuedByLiveOperator(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::ReentrancyGuardReentrantCall(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::RefundNotReady(element) => ::core::fmt::Display::fmt(element, f),
                Self::RemoteCountMismatch(element) => {
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
                Self::TooManyLocal(element) => ::core::fmt::Display::fmt(element, f),
                Self::TransferFailed(element) => ::core::fmt::Display::fmt(element, f),
                Self::Unauthorized(element) => ::core::fmt::Display::fmt(element, f),
                Self::UnbondNotReady(element) => ::core::fmt::Display::fmt(element, f),
                Self::UnbondNotRequested(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::UnsupportedKind(element) => ::core::fmt::Display::fmt(element, f),
                Self::UnsupportedNetwork(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::WitnessSerialization(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::RevertString(s) => ::core::fmt::Display::fmt(s, f),
            }
        }
    }
    impl ::core::convert::From<::std::string::String> for BetaHubErrors {
        fn from(value: String) -> Self {
            Self::RevertString(value)
        }
    }
    impl ::core::convert::From<AlreadyApproved> for BetaHubErrors {
        fn from(value: AlreadyApproved) -> Self {
            Self::AlreadyApproved(value)
        }
    }
    impl ::core::convert::From<AlreadyProcessed> for BetaHubErrors {
        fn from(value: AlreadyProcessed) -> Self {
            Self::AlreadyProcessed(value)
        }
    }
    impl ::core::convert::From<BadAnchorPayload> for BetaHubErrors {
        fn from(value: BadAnchorPayload) -> Self {
            Self::BadAnchorPayload(value)
        }
    }
    impl ::core::convert::From<BadAnchorState> for BetaHubErrors {
        fn from(value: BadAnchorState) -> Self {
            Self::BadAnchorState(value)
        }
    }
    impl ::core::convert::From<BondTooSmall> for BetaHubErrors {
        fn from(value: BondTooSmall) -> Self {
            Self::BondTooSmall(value)
        }
    }
    impl ::core::convert::From<ComponentHeldOrNotReady> for BetaHubErrors {
        fn from(value: ComponentHeldOrNotReady) -> Self {
            Self::ComponentHeldOrNotReady(value)
        }
    }
    impl ::core::convert::From<CompositionExists> for BetaHubErrors {
        fn from(value: CompositionExists) -> Self {
            Self::CompositionExists(value)
        }
    }
    impl ::core::convert::From<DuplicateComponent> for BetaHubErrors {
        fn from(value: DuplicateComponent) -> Self {
            Self::DuplicateComponent(value)
        }
    }
    impl ::core::convert::From<IncompleteRemoteComponents> for BetaHubErrors {
        fn from(value: IncompleteRemoteComponents) -> Self {
            Self::IncompleteRemoteComponents(value)
        }
    }
    impl ::core::convert::From<InvalidComponents> for BetaHubErrors {
        fn from(value: InvalidComponents) -> Self {
            Self::InvalidComponents(value)
        }
    }
    impl ::core::convert::From<InvalidHeader> for BetaHubErrors {
        fn from(value: InvalidHeader) -> Self {
            Self::InvalidHeader(value)
        }
    }
    impl ::core::convert::From<InvalidMerkleBranch> for BetaHubErrors {
        fn from(value: InvalidMerkleBranch) -> Self {
            Self::InvalidMerkleBranch(value)
        }
    }
    impl ::core::convert::From<InvalidParams> for BetaHubErrors {
        fn from(value: InvalidParams) -> Self {
            Self::InvalidParams(value)
        }
    }
    impl ::core::convert::From<KindMismatch> for BetaHubErrors {
        fn from(value: KindMismatch) -> Self {
            Self::KindMismatch(value)
        }
    }
    impl ::core::convert::From<MalformedStatement> for BetaHubErrors {
        fn from(value: MalformedStatement) -> Self {
            Self::MalformedStatement(value)
        }
    }
    impl ::core::convert::From<MalformedTx> for BetaHubErrors {
        fn from(value: MalformedTx) -> Self {
            Self::MalformedTx(value)
        }
    }
    impl ::core::convert::From<NoComposition> for BetaHubErrors {
        fn from(value: NoComposition) -> Self {
            Self::NoComposition(value)
        }
    }
    impl ::core::convert::From<NoLocalComponent> for BetaHubErrors {
        fn from(value: NoLocalComponent) -> Self {
            Self::NoLocalComponent(value)
        }
    }
    impl ::core::convert::From<NoParty> for BetaHubErrors {
        fn from(value: NoParty) -> Self {
            Self::NoParty(value)
        }
    }
    impl ::core::convert::From<NoPending> for BetaHubErrors {
        fn from(value: NoPending) -> Self {
            Self::NoPending(value)
        }
    }
    impl ::core::convert::From<NotApproved> for BetaHubErrors {
        fn from(value: NotApproved) -> Self {
            Self::NotApproved(value)
        }
    }
    impl ::core::convert::From<NotAuditor> for BetaHubErrors {
        fn from(value: NotAuditor) -> Self {
            Self::NotAuditor(value)
        }
    }
    impl ::core::convert::From<NotOnStatementChain> for BetaHubErrors {
        fn from(value: NotOnStatementChain) -> Self {
            Self::NotOnStatementChain(value)
        }
    }
    impl ::core::convert::From<NotOperator> for BetaHubErrors {
        fn from(value: NotOperator) -> Self {
            Self::NotOperator(value)
        }
    }
    impl ::core::convert::From<NotPendingUser> for BetaHubErrors {
        fn from(value: NotPendingUser) -> Self {
            Self::NotPendingUser(value)
        }
    }
    impl ::core::convert::From<PartyDead> for BetaHubErrors {
        fn from(value: PartyDead) -> Self {
            Self::PartyDead(value)
        }
    }
    impl ::core::convert::From<PartyExists> for BetaHubErrors {
        fn from(value: PartyExists) -> Self {
            Self::PartyExists(value)
        }
    }
    impl ::core::convert::From<Paused> for BetaHubErrors {
        fn from(value: Paused) -> Self {
            Self::Paused(value)
        }
    }
    impl ::core::convert::From<PendingExists> for BetaHubErrors {
        fn from(value: PendingExists) -> Self {
            Self::PendingExists(value)
        }
    }
    impl ::core::convert::From<QueuedByLiveOperator> for BetaHubErrors {
        fn from(value: QueuedByLiveOperator) -> Self {
            Self::QueuedByLiveOperator(value)
        }
    }
    impl ::core::convert::From<ReentrancyGuardReentrantCall> for BetaHubErrors {
        fn from(value: ReentrancyGuardReentrantCall) -> Self {
            Self::ReentrancyGuardReentrantCall(value)
        }
    }
    impl ::core::convert::From<RefundNotReady> for BetaHubErrors {
        fn from(value: RefundNotReady) -> Self {
            Self::RefundNotReady(value)
        }
    }
    impl ::core::convert::From<RemoteCountMismatch> for BetaHubErrors {
        fn from(value: RemoteCountMismatch) -> Self {
            Self::RemoteCountMismatch(value)
        }
    }
    impl ::core::convert::From<SafeERC20FailedOperation> for BetaHubErrors {
        fn from(value: SafeERC20FailedOperation) -> Self {
            Self::SafeERC20FailedOperation(value)
        }
    }
    impl ::core::convert::From<SkipNotReady> for BetaHubErrors {
        fn from(value: SkipNotReady) -> Self {
            Self::SkipNotReady(value)
        }
    }
    impl ::core::convert::From<StatementHashMismatch> for BetaHubErrors {
        fn from(value: StatementHashMismatch) -> Self {
            Self::StatementHashMismatch(value)
        }
    }
    impl ::core::convert::From<TargetNotProcessed> for BetaHubErrors {
        fn from(value: TargetNotProcessed) -> Self {
            Self::TargetNotProcessed(value)
        }
    }
    impl ::core::convert::From<TooManyLocal> for BetaHubErrors {
        fn from(value: TooManyLocal) -> Self {
            Self::TooManyLocal(value)
        }
    }
    impl ::core::convert::From<TransferFailed> for BetaHubErrors {
        fn from(value: TransferFailed) -> Self {
            Self::TransferFailed(value)
        }
    }
    impl ::core::convert::From<Unauthorized> for BetaHubErrors {
        fn from(value: Unauthorized) -> Self {
            Self::Unauthorized(value)
        }
    }
    impl ::core::convert::From<UnbondNotReady> for BetaHubErrors {
        fn from(value: UnbondNotReady) -> Self {
            Self::UnbondNotReady(value)
        }
    }
    impl ::core::convert::From<UnbondNotRequested> for BetaHubErrors {
        fn from(value: UnbondNotRequested) -> Self {
            Self::UnbondNotRequested(value)
        }
    }
    impl ::core::convert::From<UnsupportedKind> for BetaHubErrors {
        fn from(value: UnsupportedKind) -> Self {
            Self::UnsupportedKind(value)
        }
    }
    impl ::core::convert::From<UnsupportedNetwork> for BetaHubErrors {
        fn from(value: UnsupportedNetwork) -> Self {
            Self::UnsupportedNetwork(value)
        }
    }
    impl ::core::convert::From<WitnessSerialization> for BetaHubErrors {
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
    #[ethevent(name = "ComponentHeld", abi = "ComponentHeld(bytes32,bool)")]
    pub struct ComponentHeldFilter {
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
        name = "CompositionRegistered",
        abi = "CompositionRegistered(uint64,uint256)"
    )]
    pub struct CompositionRegisteredFilter {
        #[ethevent(indexed)]
        pub id: u64,
        pub component_count: ::ethers::core::types::U256,
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
    #[ethevent(name = "Minted", abi = "Minted(bytes32,address,uint256)")]
    pub struct MintedFilter {
        #[ethevent(indexed)]
        pub pending_id: [u8; 32],
        #[ethevent(indexed)]
        pub user: ::ethers::core::types::Address,
        pub amount: ::ethers::core::types::U256,
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
    #[ethevent(name = "PendingApproved", abi = "PendingApproved(bytes32,uint64[])")]
    pub struct PendingApprovedFilter {
        #[ethevent(indexed)]
        pub pending_id: [u8; 32],
        pub remote_lock_id: ::std::vec::Vec<u64>,
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
        name = "PendingCreated",
        abi = "PendingCreated(bytes32,address,uint64,uint64,uint64,uint64)"
    )]
    pub struct PendingCreatedFilter {
        #[ethevent(indexed)]
        pub pending_id: [u8; 32],
        #[ethevent(indexed)]
        pub user: ::ethers::core::types::Address,
        pub nonce: u64,
        pub composition_id: u64,
        pub units: u64,
        pub deadline: u64,
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
    #[ethevent(name = "PendingExpired", abi = "PendingExpired(bytes32)")]
    pub struct PendingExpiredFilter {
        #[ethevent(indexed)]
        pub pending_id: [u8; 32],
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
    #[ethevent(name = "Slashed", abi = "Slashed(bytes32,uint256,address,address)")]
    pub struct SlashedFilter {
        #[ethevent(indexed)]
        pub party_id: [u8; 32],
        pub amount: ::ethers::core::types::U256,
        pub submitter: ::ethers::core::types::Address,
        pub remainder_to: ::ethers::core::types::Address,
    }
    ///Container type for all of the contract's events
    #[derive(Clone, ::ethers::contract::EthAbiType, Debug, PartialEq, Eq, Hash)]
    pub enum BetaHubEvents {
        AnchorProcessedFilter(AnchorProcessedFilter),
        ComponentHeldFilter(ComponentHeldFilter),
        CompositionRegisteredFilter(CompositionRegisteredFilter),
        MintedFilter(MintedFilter),
        PartyRegisteredFilter(PartyRegisteredFilter),
        PendingApprovedFilter(PendingApprovedFilter),
        PendingCreatedFilter(PendingCreatedFilter),
        PendingExpiredFilter(PendingExpiredFilter),
        SlashedFilter(SlashedFilter),
    }
    impl ::ethers::contract::EthLogDecode for BetaHubEvents {
        fn decode_log(
            log: &::ethers::core::abi::RawLog,
        ) -> ::core::result::Result<Self, ::ethers::core::abi::Error> {
            if let Ok(decoded) = AnchorProcessedFilter::decode_log(log) {
                return Ok(BetaHubEvents::AnchorProcessedFilter(decoded));
            }
            if let Ok(decoded) = ComponentHeldFilter::decode_log(log) {
                return Ok(BetaHubEvents::ComponentHeldFilter(decoded));
            }
            if let Ok(decoded) = CompositionRegisteredFilter::decode_log(log) {
                return Ok(BetaHubEvents::CompositionRegisteredFilter(decoded));
            }
            if let Ok(decoded) = MintedFilter::decode_log(log) {
                return Ok(BetaHubEvents::MintedFilter(decoded));
            }
            if let Ok(decoded) = PartyRegisteredFilter::decode_log(log) {
                return Ok(BetaHubEvents::PartyRegisteredFilter(decoded));
            }
            if let Ok(decoded) = PendingApprovedFilter::decode_log(log) {
                return Ok(BetaHubEvents::PendingApprovedFilter(decoded));
            }
            if let Ok(decoded) = PendingCreatedFilter::decode_log(log) {
                return Ok(BetaHubEvents::PendingCreatedFilter(decoded));
            }
            if let Ok(decoded) = PendingExpiredFilter::decode_log(log) {
                return Ok(BetaHubEvents::PendingExpiredFilter(decoded));
            }
            if let Ok(decoded) = SlashedFilter::decode_log(log) {
                return Ok(BetaHubEvents::SlashedFilter(decoded));
            }
            Err(::ethers::core::abi::Error::InvalidData)
        }
    }
    impl ::core::fmt::Display for BetaHubEvents {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            match self {
                Self::AnchorProcessedFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::ComponentHeldFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::CompositionRegisteredFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::MintedFilter(element) => ::core::fmt::Display::fmt(element, f),
                Self::PartyRegisteredFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::PendingApprovedFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::PendingCreatedFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::PendingExpiredFilter(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::SlashedFilter(element) => ::core::fmt::Display::fmt(element, f),
            }
        }
    }
    impl ::core::convert::From<AnchorProcessedFilter> for BetaHubEvents {
        fn from(value: AnchorProcessedFilter) -> Self {
            Self::AnchorProcessedFilter(value)
        }
    }
    impl ::core::convert::From<ComponentHeldFilter> for BetaHubEvents {
        fn from(value: ComponentHeldFilter) -> Self {
            Self::ComponentHeldFilter(value)
        }
    }
    impl ::core::convert::From<CompositionRegisteredFilter> for BetaHubEvents {
        fn from(value: CompositionRegisteredFilter) -> Self {
            Self::CompositionRegisteredFilter(value)
        }
    }
    impl ::core::convert::From<MintedFilter> for BetaHubEvents {
        fn from(value: MintedFilter) -> Self {
            Self::MintedFilter(value)
        }
    }
    impl ::core::convert::From<PartyRegisteredFilter> for BetaHubEvents {
        fn from(value: PartyRegisteredFilter) -> Self {
            Self::PartyRegisteredFilter(value)
        }
    }
    impl ::core::convert::From<PendingApprovedFilter> for BetaHubEvents {
        fn from(value: PendingApprovedFilter) -> Self {
            Self::PendingApprovedFilter(value)
        }
    }
    impl ::core::convert::From<PendingCreatedFilter> for BetaHubEvents {
        fn from(value: PendingCreatedFilter) -> Self {
            Self::PendingCreatedFilter(value)
        }
    }
    impl ::core::convert::From<PendingExpiredFilter> for BetaHubEvents {
        fn from(value: PendingExpiredFilter) -> Self {
            Self::PendingExpiredFilter(value)
        }
    }
    impl ::core::convert::From<SlashedFilter> for BetaHubEvents {
        fn from(value: SlashedFilter) -> Self {
            Self::SlashedFilter(value)
        }
    }
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
    ///Container type for all input parameters for the `MAX_COMPONENTS` function with signature `MAX_COMPONENTS()` and selector `0x0ab50a6a`
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
    #[ethcall(name = "MAX_COMPONENTS", abi = "MAX_COMPONENTS()")]
    pub struct MaxComponentsCall;
    ///Container type for all input parameters for the `MAX_LOCAL_COMPONENTS` function with signature `MAX_LOCAL_COMPONENTS()` and selector `0x01548fd5`
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
    #[ethcall(name = "MAX_LOCAL_COMPONENTS", abi = "MAX_LOCAL_COMPONENTS()")]
    pub struct MaxLocalComponentsCall;
    ///Container type for all input parameters for the `SELF_NETWORK_ID` function with signature `SELF_NETWORK_ID()` and selector `0xede4754a`
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
    #[ethcall(name = "SELF_NETWORK_ID", abi = "SELF_NETWORK_ID()")]
    pub struct SelfNetworkIdCall;
    ///Container type for all input parameters for the `SOLANA_NETWORK_ID` function with signature `SOLANA_NETWORK_ID()` and selector `0xf0494276`
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
    #[ethcall(name = "SOLANA_NETWORK_ID", abi = "SOLANA_NETWORK_ID()")]
    pub struct SolanaNetworkIdCall;
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
    ///Container type for all input parameters for the `approvePending` function with signature `approvePending(uint64,uint64[])` and selector `0x6799cdb5`
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
    #[ethcall(name = "approvePending", abi = "approvePending(uint64,uint64[])")]
    pub struct ApprovePendingCall {
        pub nonce: u64,
        pub remote_lock_id: ::std::vec::Vec<u64>,
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
    ///Container type for all input parameters for the `compositionExists` function with signature `compositionExists(uint64)` and selector `0x36f88572`
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
    #[ethcall(name = "compositionExists", abi = "compositionExists(uint64)")]
    pub struct CompositionExistsCall {
        pub id: u64,
    }
    ///Container type for all input parameters for the `exerciseMint` function with signature `exerciseMint(address,uint64)` and selector `0x9b0b90ff`
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
    #[ethcall(name = "exerciseMint", abi = "exerciseMint(address,uint64)")]
    pub struct ExerciseMintCall {
        pub user: ::ethers::core::types::Address,
        pub nonce: u64,
    }
    ///Container type for all input parameters for the `expirePending` function with signature `expirePending(address,uint64)` and selector `0x015622bc`
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
    #[ethcall(name = "expirePending", abi = "expirePending(address,uint64)")]
    pub struct ExpirePendingCall {
        pub user: ::ethers::core::types::Address,
        pub nonce: u64,
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
    ///Container type for all input parameters for the `getComposition` function with signature `getComposition(uint64)` and selector `0xb2fd6cab`
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
    #[ethcall(name = "getComposition", abi = "getComposition(uint64)")]
    pub struct GetCompositionCall {
        pub id: u64,
    }
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
    ///Container type for all input parameters for the `lockLocal` function with signature `lockLocal(uint64,uint64,uint64,uint64)` and selector `0x40ec1309`
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
    #[ethcall(name = "lockLocal", abi = "lockLocal(uint64,uint64,uint64,uint64)")]
    pub struct LockLocalCall {
        pub composition_id: u64,
        pub nonce: u64,
        pub units: u64,
        pub deadline: u64,
    }
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
    ///Container type for all input parameters for the `pending` function with signature `pending(bytes32)` and selector `0x1808eeb8`
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
    #[ethcall(name = "pending", abi = "pending(bytes32)")]
    pub struct PendingCall(pub [u8; 32]);
    ///Container type for all input parameters for the `pendingRemoteAnchorTxid` function with signature `pendingRemoteAnchorTxid(bytes32)` and selector `0x2b72faff`
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
        name = "pendingRemoteAnchorTxid",
        abi = "pendingRemoteAnchorTxid(bytes32)"
    )]
    pub struct PendingRemoteAnchorTxidCall {
        pub pending_id: [u8; 32],
    }
    ///Container type for all input parameters for the `pendingRemoteLockId` function with signature `pendingRemoteLockId(bytes32)` and selector `0xa02dafef`
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
    #[ethcall(name = "pendingRemoteLockId", abi = "pendingRemoteLockId(bytes32)")]
    pub struct PendingRemoteLockIdCall {
        pub pending_id: [u8; 32],
    }
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
    ///Container type for all input parameters for the `registerComposition` function with signature `registerComposition(uint64,(uint256,address,uint256)[])` and selector `0xfeebd416`
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
        name = "registerComposition",
        abi = "registerComposition(uint64,(uint256,address,uint256)[])"
    )]
    pub struct RegisterCompositionCall {
        pub id: u64,
        pub components: ::std::vec::Vec<Component>,
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
    ///Container type for all input parameters for the `setParams` function with signature `setParams((uint64,uint64,uint64,uint64,uint256,uint256,uint256,uint256,uint256,uint16),bool)` and selector `0x09ebc914`
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
        abi = "setParams((uint64,uint64,uint64,uint64,uint256,uint256,uint256,uint256,uint256,uint16),bool)"
    )]
    pub struct SetParamsCall {
        pub p: Params,
        pub paused: bool,
    }
    ///Container type for all input parameters for the `settleMint` function with signature `settleMint(bytes32)` and selector `0x91d3a9c0`
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
    #[ethcall(name = "settleMint", abi = "settleMint(bytes32)")]
    pub struct SettleMintCall {
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
    ///Container type for all input parameters for the `token` function with signature `token()` and selector `0xfc0c546a`
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
    #[ethcall(name = "token", abi = "token()")]
    pub struct TokenCall;
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
    pub enum BetaHubCalls {
        BpsDenom(BpsDenomCall),
        KindAlive(KindAliveCall),
        KindAttest(KindAttestCall),
        KindCancel(KindCancelCall),
        KindClear(KindClearCall),
        KindMint(KindMintCall),
        KindRelease(KindReleaseCall),
        KindVeto(KindVetoCall),
        MaxComponents(MaxComponentsCall),
        MaxLocalComponents(MaxLocalComponentsCall),
        SelfNetworkId(SelfNetworkIdCall),
        SolanaNetworkId(SolanaNetworkIdCall),
        Anchors(AnchorsCall),
        ApproveOperator(ApproveOperatorCall),
        ApprovePending(ApprovePendingCall),
        ApprovedOperators(ApprovedOperatorsCall),
        CompositionExists(CompositionExistsCall),
        ExerciseMint(ExerciseMintCall),
        ExpirePending(ExpirePendingCall),
        FundRewards(FundRewardsCall),
        GetComposition(GetCompositionCall),
        Governance(GovernanceCall),
        Insurance(InsuranceCall),
        IpowHeaders(IpowHeadersCall),
        LockLocal(LockLocalCall),
        MintAttester(MintAttesterCall),
        Params(ParamsCall),
        Parties(PartiesCall),
        Paused(PausedCall),
        Pending(PendingCall),
        PendingRemoteAnchorTxid(PendingRemoteAnchorTxidCall),
        PendingRemoteLockId(PendingRemoteLockIdCall),
        ProcessAnchor(ProcessAnchorCall),
        RegisterComposition(RegisterCompositionCall),
        RegisterParty(RegisterPartyCall),
        RequestUnbond(RequestUnbondCall),
        RewardPool(RewardPoolCall),
        SetParams(SetParamsCall),
        SettleMint(SettleMintCall),
        SkipAnchor(SkipAnchorCall),
        Token(TokenCall),
        TopUpBond(TopUpBondCall),
        TotalBonds(TotalBondsCall),
        WithdrawBond(WithdrawBondCall),
    }
    impl ::ethers::core::abi::AbiDecode for BetaHubCalls {
        fn decode(
            data: impl AsRef<[u8]>,
        ) -> ::core::result::Result<Self, ::ethers::core::abi::AbiError> {
            let data = data.as_ref();
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
            if let Ok(decoded) = <MaxComponentsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::MaxComponents(decoded));
            }
            if let Ok(decoded) = <MaxLocalComponentsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::MaxLocalComponents(decoded));
            }
            if let Ok(decoded) = <SelfNetworkIdCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SelfNetworkId(decoded));
            }
            if let Ok(decoded) = <SolanaNetworkIdCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SolanaNetworkId(decoded));
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
            if let Ok(decoded) = <ApprovePendingCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ApprovePending(decoded));
            }
            if let Ok(decoded) = <ApprovedOperatorsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ApprovedOperators(decoded));
            }
            if let Ok(decoded) = <CompositionExistsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::CompositionExists(decoded));
            }
            if let Ok(decoded) = <ExerciseMintCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ExerciseMint(decoded));
            }
            if let Ok(decoded) = <ExpirePendingCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ExpirePending(decoded));
            }
            if let Ok(decoded) = <FundRewardsCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::FundRewards(decoded));
            }
            if let Ok(decoded) = <GetCompositionCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::GetComposition(decoded));
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
            if let Ok(decoded) = <IpowHeadersCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::IpowHeaders(decoded));
            }
            if let Ok(decoded) = <LockLocalCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::LockLocal(decoded));
            }
            if let Ok(decoded) = <MintAttesterCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::MintAttester(decoded));
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
            if let Ok(decoded) = <PendingCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Pending(decoded));
            }
            if let Ok(decoded) = <PendingRemoteAnchorTxidCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::PendingRemoteAnchorTxid(decoded));
            }
            if let Ok(decoded) = <PendingRemoteLockIdCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::PendingRemoteLockId(decoded));
            }
            if let Ok(decoded) = <ProcessAnchorCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::ProcessAnchor(decoded));
            }
            if let Ok(decoded) = <RegisterCompositionCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::RegisterComposition(decoded));
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
            if let Ok(decoded) = <SettleMintCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SettleMint(decoded));
            }
            if let Ok(decoded) = <SkipAnchorCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::SkipAnchor(decoded));
            }
            if let Ok(decoded) = <TokenCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::Token(decoded));
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
            if let Ok(decoded) = <WithdrawBondCall as ::ethers::core::abi::AbiDecode>::decode(
                data,
            ) {
                return Ok(Self::WithdrawBond(decoded));
            }
            Err(::ethers::core::abi::Error::InvalidData.into())
        }
    }
    impl ::ethers::core::abi::AbiEncode for BetaHubCalls {
        fn encode(self) -> Vec<u8> {
            match self {
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
                Self::MaxComponents(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::MaxLocalComponents(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SelfNetworkId(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SolanaNetworkId(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Anchors(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::ApproveOperator(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ApprovePending(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ApprovedOperators(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::CompositionExists(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ExerciseMint(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ExpirePending(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::FundRewards(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::GetComposition(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Governance(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Insurance(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::IpowHeaders(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::LockLocal(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::MintAttester(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Params(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::Parties(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::Paused(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::Pending(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::PendingRemoteAnchorTxid(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::PendingRemoteLockId(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::ProcessAnchor(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::RegisterComposition(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
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
                Self::SettleMint(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::SkipAnchor(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::Token(element) => ::ethers::core::abi::AbiEncode::encode(element),
                Self::TopUpBond(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::TotalBonds(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
                Self::WithdrawBond(element) => {
                    ::ethers::core::abi::AbiEncode::encode(element)
                }
            }
        }
    }
    impl ::core::fmt::Display for BetaHubCalls {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            match self {
                Self::BpsDenom(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindAlive(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindAttest(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindCancel(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindClear(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindMint(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindRelease(element) => ::core::fmt::Display::fmt(element, f),
                Self::KindVeto(element) => ::core::fmt::Display::fmt(element, f),
                Self::MaxComponents(element) => ::core::fmt::Display::fmt(element, f),
                Self::MaxLocalComponents(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::SelfNetworkId(element) => ::core::fmt::Display::fmt(element, f),
                Self::SolanaNetworkId(element) => ::core::fmt::Display::fmt(element, f),
                Self::Anchors(element) => ::core::fmt::Display::fmt(element, f),
                Self::ApproveOperator(element) => ::core::fmt::Display::fmt(element, f),
                Self::ApprovePending(element) => ::core::fmt::Display::fmt(element, f),
                Self::ApprovedOperators(element) => ::core::fmt::Display::fmt(element, f),
                Self::CompositionExists(element) => ::core::fmt::Display::fmt(element, f),
                Self::ExerciseMint(element) => ::core::fmt::Display::fmt(element, f),
                Self::ExpirePending(element) => ::core::fmt::Display::fmt(element, f),
                Self::FundRewards(element) => ::core::fmt::Display::fmt(element, f),
                Self::GetComposition(element) => ::core::fmt::Display::fmt(element, f),
                Self::Governance(element) => ::core::fmt::Display::fmt(element, f),
                Self::Insurance(element) => ::core::fmt::Display::fmt(element, f),
                Self::IpowHeaders(element) => ::core::fmt::Display::fmt(element, f),
                Self::LockLocal(element) => ::core::fmt::Display::fmt(element, f),
                Self::MintAttester(element) => ::core::fmt::Display::fmt(element, f),
                Self::Params(element) => ::core::fmt::Display::fmt(element, f),
                Self::Parties(element) => ::core::fmt::Display::fmt(element, f),
                Self::Paused(element) => ::core::fmt::Display::fmt(element, f),
                Self::Pending(element) => ::core::fmt::Display::fmt(element, f),
                Self::PendingRemoteAnchorTxid(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::PendingRemoteLockId(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::ProcessAnchor(element) => ::core::fmt::Display::fmt(element, f),
                Self::RegisterComposition(element) => {
                    ::core::fmt::Display::fmt(element, f)
                }
                Self::RegisterParty(element) => ::core::fmt::Display::fmt(element, f),
                Self::RequestUnbond(element) => ::core::fmt::Display::fmt(element, f),
                Self::RewardPool(element) => ::core::fmt::Display::fmt(element, f),
                Self::SetParams(element) => ::core::fmt::Display::fmt(element, f),
                Self::SettleMint(element) => ::core::fmt::Display::fmt(element, f),
                Self::SkipAnchor(element) => ::core::fmt::Display::fmt(element, f),
                Self::Token(element) => ::core::fmt::Display::fmt(element, f),
                Self::TopUpBond(element) => ::core::fmt::Display::fmt(element, f),
                Self::TotalBonds(element) => ::core::fmt::Display::fmt(element, f),
                Self::WithdrawBond(element) => ::core::fmt::Display::fmt(element, f),
            }
        }
    }
    impl ::core::convert::From<BpsDenomCall> for BetaHubCalls {
        fn from(value: BpsDenomCall) -> Self {
            Self::BpsDenom(value)
        }
    }
    impl ::core::convert::From<KindAliveCall> for BetaHubCalls {
        fn from(value: KindAliveCall) -> Self {
            Self::KindAlive(value)
        }
    }
    impl ::core::convert::From<KindAttestCall> for BetaHubCalls {
        fn from(value: KindAttestCall) -> Self {
            Self::KindAttest(value)
        }
    }
    impl ::core::convert::From<KindCancelCall> for BetaHubCalls {
        fn from(value: KindCancelCall) -> Self {
            Self::KindCancel(value)
        }
    }
    impl ::core::convert::From<KindClearCall> for BetaHubCalls {
        fn from(value: KindClearCall) -> Self {
            Self::KindClear(value)
        }
    }
    impl ::core::convert::From<KindMintCall> for BetaHubCalls {
        fn from(value: KindMintCall) -> Self {
            Self::KindMint(value)
        }
    }
    impl ::core::convert::From<KindReleaseCall> for BetaHubCalls {
        fn from(value: KindReleaseCall) -> Self {
            Self::KindRelease(value)
        }
    }
    impl ::core::convert::From<KindVetoCall> for BetaHubCalls {
        fn from(value: KindVetoCall) -> Self {
            Self::KindVeto(value)
        }
    }
    impl ::core::convert::From<MaxComponentsCall> for BetaHubCalls {
        fn from(value: MaxComponentsCall) -> Self {
            Self::MaxComponents(value)
        }
    }
    impl ::core::convert::From<MaxLocalComponentsCall> for BetaHubCalls {
        fn from(value: MaxLocalComponentsCall) -> Self {
            Self::MaxLocalComponents(value)
        }
    }
    impl ::core::convert::From<SelfNetworkIdCall> for BetaHubCalls {
        fn from(value: SelfNetworkIdCall) -> Self {
            Self::SelfNetworkId(value)
        }
    }
    impl ::core::convert::From<SolanaNetworkIdCall> for BetaHubCalls {
        fn from(value: SolanaNetworkIdCall) -> Self {
            Self::SolanaNetworkId(value)
        }
    }
    impl ::core::convert::From<AnchorsCall> for BetaHubCalls {
        fn from(value: AnchorsCall) -> Self {
            Self::Anchors(value)
        }
    }
    impl ::core::convert::From<ApproveOperatorCall> for BetaHubCalls {
        fn from(value: ApproveOperatorCall) -> Self {
            Self::ApproveOperator(value)
        }
    }
    impl ::core::convert::From<ApprovePendingCall> for BetaHubCalls {
        fn from(value: ApprovePendingCall) -> Self {
            Self::ApprovePending(value)
        }
    }
    impl ::core::convert::From<ApprovedOperatorsCall> for BetaHubCalls {
        fn from(value: ApprovedOperatorsCall) -> Self {
            Self::ApprovedOperators(value)
        }
    }
    impl ::core::convert::From<CompositionExistsCall> for BetaHubCalls {
        fn from(value: CompositionExistsCall) -> Self {
            Self::CompositionExists(value)
        }
    }
    impl ::core::convert::From<ExerciseMintCall> for BetaHubCalls {
        fn from(value: ExerciseMintCall) -> Self {
            Self::ExerciseMint(value)
        }
    }
    impl ::core::convert::From<ExpirePendingCall> for BetaHubCalls {
        fn from(value: ExpirePendingCall) -> Self {
            Self::ExpirePending(value)
        }
    }
    impl ::core::convert::From<FundRewardsCall> for BetaHubCalls {
        fn from(value: FundRewardsCall) -> Self {
            Self::FundRewards(value)
        }
    }
    impl ::core::convert::From<GetCompositionCall> for BetaHubCalls {
        fn from(value: GetCompositionCall) -> Self {
            Self::GetComposition(value)
        }
    }
    impl ::core::convert::From<GovernanceCall> for BetaHubCalls {
        fn from(value: GovernanceCall) -> Self {
            Self::Governance(value)
        }
    }
    impl ::core::convert::From<InsuranceCall> for BetaHubCalls {
        fn from(value: InsuranceCall) -> Self {
            Self::Insurance(value)
        }
    }
    impl ::core::convert::From<IpowHeadersCall> for BetaHubCalls {
        fn from(value: IpowHeadersCall) -> Self {
            Self::IpowHeaders(value)
        }
    }
    impl ::core::convert::From<LockLocalCall> for BetaHubCalls {
        fn from(value: LockLocalCall) -> Self {
            Self::LockLocal(value)
        }
    }
    impl ::core::convert::From<MintAttesterCall> for BetaHubCalls {
        fn from(value: MintAttesterCall) -> Self {
            Self::MintAttester(value)
        }
    }
    impl ::core::convert::From<ParamsCall> for BetaHubCalls {
        fn from(value: ParamsCall) -> Self {
            Self::Params(value)
        }
    }
    impl ::core::convert::From<PartiesCall> for BetaHubCalls {
        fn from(value: PartiesCall) -> Self {
            Self::Parties(value)
        }
    }
    impl ::core::convert::From<PausedCall> for BetaHubCalls {
        fn from(value: PausedCall) -> Self {
            Self::Paused(value)
        }
    }
    impl ::core::convert::From<PendingCall> for BetaHubCalls {
        fn from(value: PendingCall) -> Self {
            Self::Pending(value)
        }
    }
    impl ::core::convert::From<PendingRemoteAnchorTxidCall> for BetaHubCalls {
        fn from(value: PendingRemoteAnchorTxidCall) -> Self {
            Self::PendingRemoteAnchorTxid(value)
        }
    }
    impl ::core::convert::From<PendingRemoteLockIdCall> for BetaHubCalls {
        fn from(value: PendingRemoteLockIdCall) -> Self {
            Self::PendingRemoteLockId(value)
        }
    }
    impl ::core::convert::From<ProcessAnchorCall> for BetaHubCalls {
        fn from(value: ProcessAnchorCall) -> Self {
            Self::ProcessAnchor(value)
        }
    }
    impl ::core::convert::From<RegisterCompositionCall> for BetaHubCalls {
        fn from(value: RegisterCompositionCall) -> Self {
            Self::RegisterComposition(value)
        }
    }
    impl ::core::convert::From<RegisterPartyCall> for BetaHubCalls {
        fn from(value: RegisterPartyCall) -> Self {
            Self::RegisterParty(value)
        }
    }
    impl ::core::convert::From<RequestUnbondCall> for BetaHubCalls {
        fn from(value: RequestUnbondCall) -> Self {
            Self::RequestUnbond(value)
        }
    }
    impl ::core::convert::From<RewardPoolCall> for BetaHubCalls {
        fn from(value: RewardPoolCall) -> Self {
            Self::RewardPool(value)
        }
    }
    impl ::core::convert::From<SetParamsCall> for BetaHubCalls {
        fn from(value: SetParamsCall) -> Self {
            Self::SetParams(value)
        }
    }
    impl ::core::convert::From<SettleMintCall> for BetaHubCalls {
        fn from(value: SettleMintCall) -> Self {
            Self::SettleMint(value)
        }
    }
    impl ::core::convert::From<SkipAnchorCall> for BetaHubCalls {
        fn from(value: SkipAnchorCall) -> Self {
            Self::SkipAnchor(value)
        }
    }
    impl ::core::convert::From<TokenCall> for BetaHubCalls {
        fn from(value: TokenCall) -> Self {
            Self::Token(value)
        }
    }
    impl ::core::convert::From<TopUpBondCall> for BetaHubCalls {
        fn from(value: TopUpBondCall) -> Self {
            Self::TopUpBond(value)
        }
    }
    impl ::core::convert::From<TotalBondsCall> for BetaHubCalls {
        fn from(value: TotalBondsCall) -> Self {
            Self::TotalBonds(value)
        }
    }
    impl ::core::convert::From<WithdrawBondCall> for BetaHubCalls {
        fn from(value: WithdrawBondCall) -> Self {
            Self::WithdrawBond(value)
        }
    }
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
    ///Container type for all return fields from the `MAX_COMPONENTS` function with signature `MAX_COMPONENTS()` and selector `0x0ab50a6a`
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
    pub struct MaxComponentsReturn(pub ::ethers::core::types::U256);
    ///Container type for all return fields from the `MAX_LOCAL_COMPONENTS` function with signature `MAX_LOCAL_COMPONENTS()` and selector `0x01548fd5`
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
    pub struct MaxLocalComponentsReturn(pub ::ethers::core::types::U256);
    ///Container type for all return fields from the `SELF_NETWORK_ID` function with signature `SELF_NETWORK_ID()` and selector `0xede4754a`
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
    pub struct SelfNetworkIdReturn(pub ::ethers::core::types::U256);
    ///Container type for all return fields from the `SOLANA_NETWORK_ID` function with signature `SOLANA_NETWORK_ID()` and selector `0xf0494276`
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
    pub struct SolanaNetworkIdReturn(pub ::ethers::core::types::U256);
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
        pub composition_id: u64,
        pub component_index: u8,
        pub lock_id: u64,
        pub units: u64,
        pub challenge_until: u64,
        pub held: bool,
        pub settled: bool,
        pub pending_id: [u8; 32],
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
    ///Container type for all return fields from the `compositionExists` function with signature `compositionExists(uint64)` and selector `0x36f88572`
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
    pub struct CompositionExistsReturn(pub bool);
    ///Container type for all return fields from the `exerciseMint` function with signature `exerciseMint(address,uint64)` and selector `0x9b0b90ff`
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
    pub struct ExerciseMintReturn {
        pub minted: ::ethers::core::types::U256,
    }
    ///Container type for all return fields from the `getComposition` function with signature `getComposition(uint64)` and selector `0xb2fd6cab`
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
    pub struct GetCompositionReturn(pub ::std::vec::Vec<Component>);
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
    ///Container type for all return fields from the `lockLocal` function with signature `lockLocal(uint64,uint64,uint64,uint64)` and selector `0x40ec1309`
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
    pub struct LockLocalReturn {
        pub pending_id: [u8; 32],
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
        pub t_challenge_secs: u64,
        pub t_skip_secs: u64,
        pub refund_margin_secs: u64,
        pub unbond_delay_secs: u64,
        pub min_operator_bond: ::ethers::core::types::U256,
        pub min_auditor_bond: ::ethers::core::types::U256,
        pub slash_wei_per_unit: ::ethers::core::types::U256,
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
    ///Container type for all return fields from the `pending` function with signature `pending(bytes32)` and selector `0x1808eeb8`
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
    pub struct PendingReturn {
        pub user: ::ethers::core::types::Address,
        pub nonce: u64,
        pub composition_id: u64,
        pub units: u64,
        pub deadline: u64,
        pub approved: bool,
        pub queued_by: [u8; 32],
        pub created_at: u64,
    }
    ///Container type for all return fields from the `pendingRemoteAnchorTxid` function with signature `pendingRemoteAnchorTxid(bytes32)` and selector `0x2b72faff`
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
    pub struct PendingRemoteAnchorTxidReturn(pub ::std::vec::Vec<[u8; 32]>);
    ///Container type for all return fields from the `pendingRemoteLockId` function with signature `pendingRemoteLockId(bytes32)` and selector `0xa02dafef`
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
    pub struct PendingRemoteLockIdReturn(pub ::std::vec::Vec<u64>);
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
    ///Container type for all return fields from the `token` function with signature `token()` and selector `0xfc0c546a`
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
    pub struct TokenReturn(pub ::ethers::core::types::Address);
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
    ///`Params(uint64,uint64,uint64,uint64,uint256,uint256,uint256,uint256,uint256,uint16)`
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
        pub t_challenge_secs: u64,
        pub t_skip_secs: u64,
        pub refund_margin_secs: u64,
        pub unbond_delay_secs: u64,
        pub min_operator_bond: ::ethers::core::types::U256,
        pub min_auditor_bond: ::ethers::core::types::U256,
        pub slash_wei_per_unit: ::ethers::core::types::U256,
        pub veto_slash_wei: ::ethers::core::types::U256,
        pub veto_reward_wei: ::ethers::core::types::U256,
        pub bounty_bps: u16,
    }
    ///`Component(uint256,address,uint256)`
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
    pub struct Component {
        pub network_id: ::ethers::core::types::U256,
        pub token_id: ::ethers::core::types::Address,
        pub amount_per_unit: ::ethers::core::types::U256,
    }
}

