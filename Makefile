.PHONY: release-patch release-minor release-major dist-plan dist-build

# Release: bump version with cargo-release, then push tag to trigger CI
release-patch:
	cargo release patch --execute
	git push origin --tags

release-minor:
	cargo release minor --execute
	git push origin --tags

release-major:
	cargo release major --execute
	git push origin --tags

# Preview what dist will build
dist-plan:
	cargo dist plan

# Test local build
dist-build:
	cargo dist build
