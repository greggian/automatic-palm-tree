"""Shared pytest fixtures for NHC parser tests."""
from __future__ import annotations

import pathlib
import pytest

TEST_DATA = pathlib.Path(__file__).parent.parent / "test_data"


def _load(name: str) -> str:
    return (TEST_DATA / name).read_text()


@pytest.fixture
def fstadv_001() -> str:
    return _load("ep042025.fstadv.001.txt")


@pytest.fixture
def fstadv_008() -> str:
    return _load("ep042025.fstadv.008.txt")


@pytest.fixture
def fstadv_014() -> str:
    return _load("ep042025.fstadv.014.txt")


@pytest.fixture
def public_001() -> str:
    return _load("ep042025.public.001.txt")


@pytest.fixture
def public_008() -> str:
    return _load("ep042025.public.008.txt")


@pytest.fixture
def public_014() -> str:
    return _load("ep042025.public.014.txt")


@pytest.fixture
def discus_001() -> str:
    return _load("ep042025.discus.001.txt")


@pytest.fixture
def discus_008() -> str:
    return _load("ep042025.discus.008.txt")


@pytest.fixture
def discus_014() -> str:
    return _load("ep042025.discus.014.txt")


@pytest.fixture
def wndprb_001() -> str:
    return _load("ep042025.wndprb.001.txt")


@pytest.fixture
def wndprb_008() -> str:
    return _load("ep042025.wndprb.008.txt")


@pytest.fixture
def wndprb_014() -> str:
    return _load("ep042025.wndprb.014.txt")
