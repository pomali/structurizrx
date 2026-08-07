# 1. Use PostgreSQL for the Shop database

Date: 2026-06-01

## Status

Accepted

## Context

The Shop API needs a relational store for orders and inventory with strong
consistency guarantees and mature tooling.

## Decision

We will use PostgreSQL as the primary database for the Shop system.

## Consequences

The team standardizes on one relational engine across services, simplifying
operational tooling and backup strategy.
