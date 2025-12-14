# Security Policy

This repository ships a reference UBL kernel and trust primitives.

## Reporting
If you find a vulnerability, please contact maintainers privately.

## Hardening checklist
- Set `UBL_API_KEY` in production.
- Run behind TLS.
- Restrict registry mutation endpoints.
- Rotate signing keys and keep them offline when possible.
