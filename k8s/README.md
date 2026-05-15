# TravelGraph Kubernetes

## Kustomize

Local:

```sh
kubectl apply -k k8s/overlays/local/
kubectl -n travelgraph get pods
```

Production-style overlay:

```sh
kubectl apply -k k8s/overlays/prod/
```

The manifests use GHCR images by default. Edit the `images` block in the overlay
to match your GitHub owner, for example
`ghcr.io/<owner>/travelgraph-router:<sha>`.

The local overlay enables arbitrary GraphQL query text on the router so the demo
can be exercised without a persisted-query manifest. The base/prod path keeps
persisted queries enforced.

## Secrets

For the demo, `k8s/base/secrets.yaml` is a normal Kubernetes `Secret` with
development values. Production should replace it with a SOPS-encrypted Secret
that has the same name and keys:

- `postgres-password`
- `jwt-signing-key`
- `identity-hmac-key`
- `grafana-admin-password`

SOPS is the documented choice here because it works cleanly with GitOps and lets
the repo store encrypted YAML without requiring a cluster-side controller.

## Ingress

The base Ingress uses `travelgraph.local`:

- GraphQL router: `/graphql`
- Grafana: `/grafana`

For kind or minikube, point `travelgraph.local` at your ingress controller IP,
then use:

```sh
curl -H 'Host: travelgraph.local' http://127.0.0.1/graphql
```

## Network Policy

`subgraphs-from-router-only` allows inbound traffic to subgraph pods only from
the router pod. The router, registry, Postgres, Redis, and Grafana keep their
default namespace-local behavior.

## Helm

```sh
helm install travelgraph k8s/helm/travelgraph
helm upgrade travelgraph k8s/helm/travelgraph --set global.imageTag=<sha>
```

