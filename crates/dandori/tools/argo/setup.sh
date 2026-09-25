#!/bin/sh
# Sets up what tools/argo/run.mjs runs the workflows on: a kind cluster `dandori` with Argo
# Workflows, no artifact store (the tests keep nothing but parameters), a role that lets a
# workflow start another, and the mock that serves the scenarios' answers (mock.mjs).
# Run it again to bring the mock up to date. Needs docker, kind and kubectl.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
version=v4.1.4
k="kubectl --context kind-dandori"

if ! kind get clusters 2>/dev/null | grep -qx dandori; then
  kind create cluster --name dandori --wait 120s
fi
for image in quay.io/argoproj/workflow-controller:$version quay.io/argoproj/argoexec:$version quay.io/argoproj/argocli:$version node:24-alpine busybox:1.37; do
  docker image inspect "$image" >/dev/null 2>&1 || docker pull -q "$image" >/dev/null
  kind load docker-image "$image" --name dandori >/dev/null
done

$k create namespace argo --dry-run=client -o yaml | $k apply -f - >/dev/null
$k apply --server-side -n argo -f "https://github.com/argoproj/argo-workflows/releases/download/$version/quick-start-minimal.yaml" >/dev/null
$k -n argo patch configmap workflow-controller-configmap --type json -p '[{"op":"remove","path":"/data/artifactRepository"}]' >/dev/null 2>&1 || true
# the runner reads every workflow's end, so none is deleted before it has; a task's workflow
# must be able to start while the workflows that wait for it run
$k -n argo patch configmap workflow-controller-configmap --type json -p '[{"op":"remove","path":"/data/retentionPolicy"}]' >/dev/null 2>&1 || true
$k -n argo patch configmap workflow-controller-configmap --type json -p '[{"op":"remove","path":"/data/namespaceParallelism"}]' >/dev/null 2>&1 || true
$k -n argo delete configmap artifact-repositories --ignore-not-found >/dev/null
$k -n argo delete deploy minio httpbin --ignore-not-found >/dev/null
$k -n argo rollout restart deploy/workflow-controller >/dev/null
$k -n argo rollout status deploy/workflow-controller --timeout=180s >/dev/null

# a task with `workflow template` creates a Workflow and watches it
$k -n argo apply -f - >/dev/null <<'YAML'
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: dandori-child-workflows
rules:
- apiGroups: ["argoproj.io"]
  resources: ["workflows"]
  verbs: ["create", "get", "list", "watch", "patch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: dandori-child-workflows
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: dandori-child-workflows
subjects:
- kind: ServiceAccount
  name: default
  namespace: argo
YAML

$k -n argo create configmap dandori-mock --from-file=mock.mjs="$here/mock.mjs" --dry-run=client -o yaml | $k apply -f - >/dev/null
$k -n argo apply -f - >/dev/null <<'YAML'
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dandori-mock
spec:
  replicas: 1
  selector:
    matchLabels: {app: dandori-mock}
  template:
    metadata:
      labels: {app: dandori-mock}
    spec:
      containers:
      - name: mock
        image: node:24-alpine
        imagePullPolicy: IfNotPresent
        command: ["node", "/mock/mock.mjs"]
        ports: [{containerPort: 8080}]
        volumeMounts: [{name: code, mountPath: /mock}]
      volumes:
      - name: code
        configMap: {name: dandori-mock}
---
apiVersion: v1
kind: Service
metadata:
  name: dandori-mock
spec:
  selector: {app: dandori-mock}
  ports: [{port: 80, targetPort: 8080}]
YAML
$k -n argo rollout restart deploy/dandori-mock >/dev/null
$k -n argo rollout status deploy/dandori-mock --timeout=120s >/dev/null
echo "the cluster dandori is ready"
