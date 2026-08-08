# 2. Write the API in Rust

Date: 2026-06-15

## Status

Accepted

## Context

The API handles the checkout hot path and needs predictable latency under
load, plus a small deployable artifact.

## Decision

We will implement the API container in Rust rather than a dynamic-language
framework.

## Consequences

Faster, more predictable request handling at the cost of a steeper
onboarding curve for new contributors.
