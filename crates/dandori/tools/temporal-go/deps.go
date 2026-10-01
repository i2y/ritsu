// The packages that the Go dandori writes for Temporal can import, kept required by this module:
// `go mod tidy` drops a requirement that no package of the module imports, and the runner's
// build (./build) copies go.mod and go.sum into the module where it builds the generated
// packages, which are not here when the module is tidied. Nothing calls this package.
package temporalgo

import (
	_ "github.com/anthropics/anthropic-sdk-go"
	_ "github.com/anthropics/anthropic-sdk-go/option"
	_ "github.com/aws/aws-sdk-go-v2/aws"
	_ "github.com/aws/aws-sdk-go-v2/aws/retry"
	_ "github.com/aws/aws-sdk-go-v2/config"
	_ "github.com/aws/aws-sdk-go-v2/service/lambda"
	_ "github.com/aws/aws-sdk-go-v2/service/lambda/types"
	_ "github.com/aws/aws-sdk-go-v2/service/sns"
	_ "github.com/aws/aws-sdk-go-v2/service/sns/types"
	_ "github.com/aws/aws-sdk-go-v2/service/sqs"
	_ "github.com/aws/aws-sdk-go-v2/service/sqs/types"
	_ "github.com/aws/smithy-go"
	_ "github.com/openai/openai-go/v3"
	_ "github.com/openai/openai-go/v3/option"
	_ "github.com/openai/openai-go/v3/packages/param"
	_ "github.com/openai/openai-go/v3/responses"
	_ "github.com/openai/openai-go/v3/shared"
	_ "go.temporal.io/api/common/v1"
	_ "go.temporal.io/api/enums/v1"
	_ "go.temporal.io/api/history/v1"
	_ "go.temporal.io/api/serviceerror"
	_ "go.temporal.io/api/workflowservice/v1"
	_ "go.temporal.io/sdk/activity"
	_ "go.temporal.io/sdk/client"
	_ "go.temporal.io/sdk/converter"
	_ "go.temporal.io/sdk/log"
	_ "go.temporal.io/sdk/temporal"
	_ "go.temporal.io/sdk/testsuite"
	_ "go.temporal.io/sdk/worker"
	_ "go.temporal.io/sdk/workflow"
)
