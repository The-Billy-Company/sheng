`release-please-config.json` named the package, and that one line is why no
release here has ever been cut by the bot that exists to cut them.

With `include-component-in-tag` off, release-please writes a standalone release
PR's body with no component in it, and names the branch
`release-please--branches--main` with no component either. Then, on merge, before
it will tag anything, it compares that empty component against
`component || package-name` - so a `package-name` here makes the two halves of
its own bookkeeping disagree permanently. Every merge logged
`PR component: undefined does not match configured component: sheng` and
returned without creating the tag or the release.

That is worse than a missed release, because it wedges: an untagged merged
release PR makes the *next* run abort before it opens anything, so the queue
stops until someone relabels the old PR by hand. v0.2.0 needed the tag and the
release typed by a person, and the tag is what `release.yml` waits on - so the
crate sat unpublished while main already called itself 0.2.0.

The name bought nothing back. With the component out of the tag, the tag is
`vX.Y.Z` and the release is titled `vX.Y.Z` whatever the package is called. The
config comment now says so, because the field looks harmless and reads exactly
like something a package config ought to declare.
