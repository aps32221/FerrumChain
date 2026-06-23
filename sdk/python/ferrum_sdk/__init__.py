"""Ferrum SDK — typed thin wrapper over substrate-interface."""
from .client import (
    FerrumClient,
    DEFAULT_ENDPOINT,
    IdentityNs,
    CredentialNs,
    TaxNs,
    TreasuryNs,
    FederationNs,
    InteropNs,
)
from . import helpers
from .helpers import POOLS

__all__ = [
    "FerrumClient",
    "DEFAULT_ENDPOINT",
    "IdentityNs",
    "CredentialNs",
    "TaxNs",
    "TreasuryNs",
    "FederationNs",
    "InteropNs",
    "helpers",
    "POOLS",
]
__version__ = "0.1.0"
