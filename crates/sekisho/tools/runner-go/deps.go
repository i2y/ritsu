// The modules the Go sekisho generates imports, kept required by this module: cedar-go, which the
// generated package asks in its own process (`--authorizer cedar`), and the AWS SDK's client of
// Verified Permissions, which the code of `--authorizer avp` calls. `go mod tidy` drops a
// requirement no package of the module imports, and tests/go.rs copies go.mod and go.sum into the
// module where it builds the generated packages, which are not here when the module is tidied.
// Nothing calls this package.
package sekishorunner

import (
	_ "github.com/aws/aws-sdk-go-v2/aws"
	_ "github.com/aws/aws-sdk-go-v2/service/verifiedpermissions"
	_ "github.com/aws/aws-sdk-go-v2/service/verifiedpermissions/types"
	_ "github.com/cedar-policy/cedar-go"
	_ "github.com/cedar-policy/cedar-go/types"
)
