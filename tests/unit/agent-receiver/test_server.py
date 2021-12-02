#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# Copyright (C) 2019 tribe29 GmbH - License: GNU General Public License v2
# This file is part of Checkmk (https://checkmk.com). It is subject to the terms and
# conditions defined in the file COPYING, which is part of this source code package.

import io
import os
from pathlib import Path
from unittest import mock

import pytest
from agent_receiver.server import app
from fastapi.testclient import TestClient
from pytest_mock import MockerFixture


@pytest.fixture(autouse=True)
def mock_paths(tmp_path: Path):
    with mock.patch("agent_receiver.server.AGENT_OUTPUT_DIR", tmp_path), mock.patch(
        "agent_receiver.server.REGISTRATION_REQUESTS", tmp_path
    ):
        yield


@pytest.fixture(autouse=True)
def deactivate_certificate_validation(mocker: MockerFixture) -> None:
    mocker.patch(
        "agent_receiver.certificates._invalid_certificate_response",
        lambda _h: None,
    )


@pytest.fixture(name="client")
def fixture_client() -> TestClient:
    return TestClient(app)


def test_agent_data_no_host(client: TestClient) -> None:
    mock_file = io.StringIO("mock file")
    response = client.post(
        "/agent_data/1234",
        headers={"certificate": "irrelevant"},
        files={"monitoring_data": ("filename", mock_file)},
    )
    assert response.status_code == 403
    assert response.json() == {"detail": "Host is not registered"}


def test_agent_data_success(
    tmp_path: Path,
    client: TestClient,
) -> None:
    mock_file = io.StringIO("mock file")

    source = tmp_path / "1234"
    target_dir = tmp_path / "hostname"
    os.mkdir(target_dir)
    source.symlink_to(target_dir)

    response = client.post(
        "/agent_data/1234",
        headers={"certificate": "irrelevant"},
        files={"monitoring_data": ("filename", mock_file)},
    )

    file_path = tmp_path / "hostname" / "received-output"
    assert file_path.exists()

    assert response.status_code == 204


def test_agent_data_move_error(
    tmp_path: Path,
    caplog,
    client: TestClient,
) -> None:
    mock_file = io.StringIO("mock file")

    os.mkdir(tmp_path / "READY")
    Path(tmp_path / "READY" / "1234.json").touch()
    os.mkdir(tmp_path / "DISCOVERABLE")

    source = tmp_path / "1234"
    target_dir = tmp_path / "hostname"
    os.mkdir(target_dir)
    source.symlink_to(target_dir)

    with mock.patch("agent_receiver.server.Path.rename") as move_mock:
        move_mock.side_effect = FileNotFoundError()
        response = client.post(
            "/agent_data/1234",
            headers={"certificate": "irrelevant"},
            files={"monitoring_data": ("filename", mock_file)},
        )

    assert response.status_code == 204
    assert caplog.records[0].message == "uuid=1234 Agent data saved"


def test_agent_data_move_ready(
    tmp_path: Path,
    client: TestClient,
) -> None:
    mock_file = io.StringIO("mock file")

    os.mkdir(tmp_path / "READY")
    Path(tmp_path / "READY" / "1234.json").touch()
    os.mkdir(tmp_path / "DISCOVERABLE")

    source = tmp_path / "1234"
    target_dir = tmp_path / "hostname"
    os.mkdir(target_dir)
    source.symlink_to(target_dir)

    client.post(
        "/agent_data/1234",
        headers={"certificate": "irrelevant"},
        files={"monitoring_data": ("filename", mock_file)},
    )

    registration_request = tmp_path / "DISCOVERABLE" / "1234.json"
    assert registration_request.exists()
