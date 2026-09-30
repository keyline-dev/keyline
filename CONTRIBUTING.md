# Contributing to keyline

Thanks for helping. Issues, bug reports and pull requests are welcome. Before a large change, open an issue to talk it over, so your time isn't spent on something that won't be merged.

**Every contribution is made under the [contributor agreement](#contributor-agreement) below: you assign the copyright in your contribution to the owner of keyline.** Read it before you open a pull request; opening one means you agree to it.

## Making a change

- Build and run the server: `cargo run --release`. Tests: `cargo test`. The README's [Development](README.md#development) section has the rest.
- A change comes with tests: unit tests beside the code and end-to-end tests in `tests/`. Rendered images are compared with the reference images in `tests/golden/<os>/`; update those only when a change is meant to alter how something looks, and say so in the pull request.
- Before you push, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` must pass.
- New dependencies need the owner's approval first, and must be under a permissive license (MIT, Apache-2.0, BSD, OFL and the like); no GPL or other copyleft.
- The scene format and the tools are documented in [docs/](docs/); a change to either updates its doc in the same pull request.

## Contributor agreement

This agreement is between you and Yuval Tal ("the Owner"), who owns keyline and licenses it under the PolyForm Shield License 1.0.0. "Your Contribution" means anything you submit to keyline, in a pull request, an issue, a patch or any other way, including code, documentation, images and tests.

By submitting Your Contribution, you agree that:

1. **Assignment.** You assign to the Owner all right, title and interest, including copyright, in Your Contribution, worldwide. The Owner may use, change, license and relicense it on any terms, including commercially and under licenses other than the one keyline uses today, and may transfer these rights to anyone.
2. **License, where assignment isn't possible.** Where the law doesn't let you assign some right, you grant the Owner a perpetual, worldwide, irrevocable, exclusive, royalty-free, transferable license, with the right to sublicense, to exercise that right in every way the assignment would have allowed.
3. **Moral rights.** To the extent the law allows, you waive, and agree not to assert, any moral rights in Your Contribution against the Owner or anyone licensed by the Owner.
4. **Patents.** You grant the Owner, and everyone who receives keyline from the Owner, a perpetual, worldwide, irrevocable, royalty-free patent license to make, use, sell and distribute Your Contribution, alone or as part of keyline.
5. **It's yours to give.** Your Contribution is your original work and you have the right to agree to this. If your employer or anyone else has rights in it, you have their permission. If any part comes from someone else, you say so in the pull request, with its source and license.
6. **No obligation.** The Owner doesn't have to use Your Contribution, and nothing here obliges anyone to pay you. You get no rights in keyline beyond those the PolyForm Shield License gives everyone.

Your Contribution is provided as is, without warranties.

If you can't agree to this, please open an issue describing the change instead of a pull request.
