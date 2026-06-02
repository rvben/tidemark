# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-06-02

### Added

- full clispec v0.1 conformance (100/100) plus robustness ([8f15f9d](https://github.com/rvben/tidemark/commit/8f15f9dbcacc570e2b93bab483d0ecd021cdd1ec))
- output rendering with pagination and field selection ([99191f5](https://github.com/rvben/tidemark/commit/99191f5523f31d14e8f893e6e9eb713536b591e5))
- clispec schema introspection ([02abd4d](https://github.com/rvben/tidemark/commit/02abd4de798c50f7ef0fdf84544df10e4b6a2b79))
- ref resolution for labels, files, and current tree ([da872e3](https://github.com/rvben/tidemark/commit/da872e3cd07a87bea880ef398f9e54b1b2c617e7))
- labeled snapshot store with idempotency and conflict detection ([f5ae330](https://github.com/rvben/tidemark/commit/f5ae330324938f4506d50a750b53778a67f23c91))
- diff engine with rename detection and unified content diff ([cd17acb](https://github.com/rvben/tidemark/commit/cd17acb8bc967586803696841937c6996e048a09))
- snap pipeline composing walk and hash ([33948d0](https://github.com/rvben/tidemark/commit/33948d046e01a19093fad3399cee6834a25374d3))
- blake3 content hashing and symlink capture ([4b891f3](https://github.com/rvben/tidemark/commit/4b891f3ccb5dd1f7f07ee2e028f78291ecd37111))
- directory walker with gitignore support ([c673418](https://github.com/rvben/tidemark/commit/c673418e61df5294e76ac138beb75eff1cb09307))
- manifest model with deterministic merkle digest ([d9f7b59](https://github.com/rvben/tidemark/commit/d9f7b59af9e2aa127eaafd46ec2e7d0b9b14e871))
- error kinds with retryable flag ([1c508b4](https://github.com/rvben/tidemark/commit/1c508b47be3e9048f6ff560b7ddf31c1eeadc23c))

### Fixed

- flush stdout/stderr before process::exit ([c4798a4](https://github.com/rvben/tidemark/commit/c4798a422c72e88485866b2399122789d8c27e2b))
