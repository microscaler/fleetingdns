# Infra Directory

This tree is managed by Flux in the **infra cluster**.

* Every YAML file starts with a comment block describing the expected content.
* Apply order is governed by `kustomization.yaml`.
* Secret manifests should live **outside** the repo or be sealed via KSOPS.
