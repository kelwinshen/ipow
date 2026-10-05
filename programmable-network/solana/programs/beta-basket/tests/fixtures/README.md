# Test fixtures

`mpl_token_metadata.so`: Metaplex Token Metadata as deployed on Solana
mainnet, program `metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`, dumped on
2026-10-05 with

    solana program dump metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s \
      programs/beta-basket/tests/fixtures/mpl_token_metadata.so \
      --url https://api.mainnet-beta.solana.com

793,991 bytes, sha256
`31f0a627dba051a938de650464e55cc5397a4be0fd496929c1f9cf02fe5e9011`.
The basket program's tests (`tests/test_beta_basket.rs`) and the SDK's
(`packages/sdk/test/solanaBeta.test.ts`) run it, since a basket's token is
named through it (E4, docs/drafts/ipow-beta-app.md). Dump it again to pick
up a newer deployment; the tests need nothing it added since.
