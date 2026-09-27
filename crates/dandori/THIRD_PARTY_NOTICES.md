# Third-party notices

The examples carry cut-down copies of two API descriptions, so that the checker can hold their
tasks to them. Each copy says in its own metadata where it comes from and that only the
operations the examples call, and the shapes those reach, are kept
([tools/specs/trim.py](tools/specs/trim.py) makes them). They keep the licenses of their
originals.

## Stripe's OpenAPI document

[examples/hotel/specs/stripe.json](examples/hotel/specs/stripe.json) is a cut-down copy of
`openapi/spec3.json` of [github.com/stripe/openapi](https://github.com/stripe/openapi), under the
MIT License:

```
The MIT License

Copyright (c) 2011- Stripe, Inc. (https://stripe.com)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

## The Smithy models of Amazon SNS and Amazon SQS

[examples/fulfillment/specs/sns.json](examples/fulfillment/specs/sns.json) and
[examples/fulfillment/specs/sqs.json](examples/fulfillment/specs/sqs.json) are cut-down copies of
`models/sns/service/2010-03-31/sns-2010-03-31.json` and
`models/sqs/service/2012-11-05/sqs-2012-11-05.json` of
[github.com/aws/api-models-aws](https://github.com/aws/api-models-aws), under the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)). Its NOTICE reads:

```
Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
```
