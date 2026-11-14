package bundle

import _ "embed"

//go:embed docs/USAGE.md
var UsageGuide string

//go:embed LICENSE
var License string

//go:embed THIRD_PARTY_NOTICES.md
var Notices string
