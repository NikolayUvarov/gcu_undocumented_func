#!/usr/bin/env python3
"""Wire-level checks of the Effector fixture; these do not test a MIND agent."""

import hashlib
import http.client
import json
import ssl
import threading
import time
import unittest

from effector_server import CONTRACT, FILE_LIMIT, JSON_LIMIT, EffectorFixture


class EffectorServerTests(unittest.TestCase):
    def setUp(self):
        self.fixture = EffectorFixture().start()
        self.context = ssl.create_default_context(cafile=str(self.fixture.certificate))
        self.context.minimum_version = ssl.TLSVersion.TLSv1_3

    def tearDown(self):
        self.fixture.close()
        self.assertEqual(list(self.fixture.errors), [])

    def connection(self, context=None):
        return http.client.HTTPSConnection("127.0.0.1", self.fixture.port, context=context or self.context, timeout=3)

    def request(self, method, route, body=None, *, token=None, raw=False, connection=None, headers=None):
        own = connection is None
        connection = connection or self.connection()
        request_headers = {"Authorization": "Bearer " + (self.fixture.token if token is None else token)}
        if body is not None and not raw:
            body = json.dumps(body).encode()
            request_headers["Content-Type"] = "application/json"
        request_headers.update(headers or {})
        try:
            connection.request(method, self.fixture.prefix + route, body, request_headers)
            response = connection.getresponse()
            return response.status, response.read(), dict(response.getheaders())
        finally:
            if own:
                connection.close()

    def heartbeat(self, **extra):
        body = {"agent_id": "mind-test", "os_version": "mindcore", "arch": "x64",
                "agent_version": "0.1.0", "programs": [], "busy": False}
        body.update(extra)
        self.assertEqual(self.request("POST", "/api/heartbeat", body)[0], 200)

    def poll(self, agent="mind-test", query=""):
        status, body, _ = self.request("GET", "/api/commands/poll?agent_id=" + agent + query)
        self.assertEqual(status, 200)
        return json.loads(body)

    def test_contract_tls_pin_auth_and_prefix(self):
        self.assertEqual(CONTRACT, "effector-agent 1.0")
        connection = self.connection()
        connection.connect()
        self.assertEqual(connection.sock.version(), "TLSv1.3")
        self.assertEqual(hashlib.sha256(connection.sock.getpeercert(binary_form=True)).hexdigest(), self.fixture.leaf_pin)
        connection.close()
        self.assertEqual(self.request("GET", "/api/config?platform=mindcore-x64", token="wrong")[0], 401)
        connection = self.connection()
        connection.request("GET", "/api/config", headers={"Authorization": "Bearer " + self.fixture.token})
        self.assertEqual(connection.getresponse().status, 404)
        connection.close()
        self.assertEqual(json.loads(self.request("GET", "/api/config?platform=mindcore-x64")[1]), {"programs": []})
        self.assertNotIn(self.fixture.token, repr(self.fixture.requests))

    def test_connection_reuse_and_native_bootstrap(self):
        connection = self.connection()
        for route in ("/api/config?platform=mindcore-arm64", "/api/agent_config"):
            self.assertEqual(self.request("GET", route, connection=connection)[0], 200)
        status, body, _ = self.request("POST", "/api/heartbeat", {"agent_id": "mind-test"}, connection=connection)
        self.assertEqual((status, json.loads(body)), (200, {"status": "ok"}))
        self.assertEqual(self.request("GET", "/api/commands/poll?agent_id=mind-test", connection=connection)[0], 200)
        connection.close()

    def test_denial_ack_retry_and_duplicate_delivery(self):
        self.heartbeat()
        command = self.fixture.queue("mind-test", "future_unknown_action", {"example": True})
        self.assertEqual(self.poll()["commands"][0]["id"], command)
        result = json.dumps({"status": "denied", "exit_code": 1, "error": "action_not_allowed"})
        self.fixture.inject("/api/commands/ack", "ack_500")
        ack = {"command_id": command, "result": result}
        self.assertEqual(self.request("POST", "/api/commands/ack", ack)[0], 500)
        self.assertIsNone(self.fixture.operation(command))
        self.fixture.redeliver(command)
        self.assertEqual(self.poll()["commands"][0]["id"], command)
        self.assertEqual(self.request("POST", "/api/commands/ack", ack)[0], 200)
        self.assertEqual(self.request("POST", "/api/commands/ack", ack)[0], 200)
        self.assertEqual(self.fixture.operation(command)["status"], "denied")
        ack["result"] = json.dumps({"status": "completed", "exit_code": 0})
        self.assertEqual(self.request("POST", "/api/commands/ack", ack)[0], 409)
        self.assertEqual(self.poll()["commands"], [])

    def test_ack_requires_sent_command_and_string_result(self):
        self.heartbeat()
        command = self.fixture.queue("mind-test", "collect_logs")
        ack = {"command_id": command, "result": {"status": "completed"}}
        self.assertEqual(self.request("POST", "/api/commands/ack", ack)[0], 400)
        ack["result"] = json.dumps(ack["result"])
        self.assertEqual(self.request("POST", "/api/commands/ack", ack)[0], 409)
        self.poll()
        ack["result"] = json.dumps({"status": "completed", "exit_code": 2})
        self.assertEqual(self.request("POST", "/api/commands/ack", ack)[0], 200)
        self.assertEqual(self.fixture.operation(command)["status"], "failed")

    def test_activation_capacity_cancel_and_agent_isolation(self):
        self.heartbeat()
        self.fixture.activated = False
        command = self.fixture.queue("mind-test", "restart_service")
        self.assertEqual(self.poll(), {"commands": [], "parallel_limit": 1, "activation_required": True})
        self.fixture.activated = True
        self.heartbeat(busy=True)
        self.fixture.queue("other-agent", "restart_service")
        cancel = self.fixture.queue("mind-test", "cancel_command", command)
        self.assertEqual(self.poll(query="&at_capacity=1")["commands"][0]["id"], cancel)
        self.assertEqual(self.poll()["commands"], [])
        self.heartbeat(busy=False)
        self.assertEqual(self.poll()["commands"][0]["id"], command)

    def test_stream_wakeup_and_clean_eof(self):
        self.heartbeat()
        connection = self.connection()
        connection.request("GET", "/effector/api/commands/stream?agent_id=mind-test",
                           headers={"Authorization": "Bearer " + self.fixture.token, "Accept": "text/event-stream"})
        response = connection.getresponse()
        self.assertEqual(response.status, 200)
        self.assertEqual(response.readline(), b": connected\n")
        self.fixture.queue("mind-test", "collect_logs")
        self.assertIn(b"event: command\ndata: ready\n\n", response.read())
        connection.close()

    def test_stream_cut_recovers_on_new_connection(self):
        self.heartbeat()
        self.fixture.inject("/api/commands/stream", "cut_stream")
        with self.assertRaises(http.client.IncompleteRead):
            self.request("GET", "/api/commands/stream?agent_id=mind-test")
        status, body, _ = self.request("GET", "/api/commands/stream?agent_id=mind-test")
        self.assertEqual((status, body), (200, b": connected\n\n"))

    def test_poll_delay_does_not_block_heartbeat_and_drop_is_recoverable(self):
        self.heartbeat()
        self.fixture.inject("/api/commands/poll", "delay", 0.25)
        answers = []
        worker = threading.Thread(target=lambda: answers.append(self.poll()))
        worker.start()
        deadline = time.monotonic() + 2
        while not any("poll" in request["path"] for request in self.fixture.requests):
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.005)
        self.heartbeat()
        self.assertTrue(worker.is_alive(), "heartbeat waited for delayed poll")
        worker.join(timeout=3)
        self.assertEqual(answers, [{"commands": [], "parallel_limit": 1}])
        self.fixture.inject("/api/commands/poll", "drop")
        with self.assertRaises(http.client.RemoteDisconnected):
            self.poll()
        self.assertEqual(self.poll()["commands"], [])

    def test_delivery_integrity_scope_and_cut_download(self):
        self.heartbeat()
        data = b"a diagnostic script\n" * 100
        command = self.fixture.queue("mind-test", "deliver_file")
        self.fixture.publish(data, command_id=command)
        route = "/api/commands/file?agent_id=mind-test&command_id=" + command
        self.assertEqual(self.request("GET", route)[0], 409)
        self.poll()
        self.assertEqual(self.request("GET", route.replace("mind-test", "other"))[0], 409)
        self.fixture.inject("/api/commands/file", "cut_download", 27)
        with self.assertRaises(http.client.IncompleteRead):
            self.request("GET", route)
        self.assertEqual(self.request("GET", route)[1], data)

    def test_an_update_bundle_is_served_for_its_command(self):
        self.heartbeat()
        bundle = bytes(range(256)) * (FILE_LIMIT // 128)  # over a delivery's bound, as a release is
        command = self.fixture.queue("mind-test", "update_agent", {"size": len(bundle), "sha256": hashlib.sha256(bundle).hexdigest()})
        with self.assertRaises(ValueError):
            self.fixture.publish(bundle, command_id=command)
        self.fixture.publish(bundle, command_id=command, bundle=True)
        route = "/api/commands/file?agent_id=mind-test&command_id=" + command
        self.assertEqual(self.request("GET", route)[0], 409, "not before it is sent")
        self.poll()
        self.assertEqual(self.request("GET", route)[1], bundle)
        # The answer comes after the agent's restart, while the command is still the one sent.
        result = json.dumps({"status": "completed", "exit_code": 0, "stdout": "updated 6, slot B confirmed", "stderr": "", "truncated": False, "run_as": "effector"})
        self.assertEqual(self.request("POST", "/api/commands/ack", {"command_id": command, "result": result})[0], 200)
        self.assertEqual(self.fixture.operation(command)["result"]["stdout"], "updated 6, slot B confirmed")

    def test_upload_digest_and_package_exchange(self):
        self.heartbeat()
        data = b"native diagnostic output\x00"
        command = self.fixture.queue("mind-test", "collect_file")
        self.poll()
        route = f"/api/commands/file?agent_id=mind-test&command_id={command}&sha256="
        self.assertEqual(self.request("POST", route + "0" * 64, data, raw=True)[0], 400)
        status, body, _ = self.request("POST", route + hashlib.sha256(data).hexdigest(), data, raw=True)
        self.assertEqual(status, 201)
        self.assertEqual(json.loads(body)["size"], len(data))
        self.assertEqual(self.fixture.uploads[command], data)
        self.fixture.publish(data, package_id="package-1")
        self.assertEqual(self.request("GET", "/api/deployment/download/package-1")[1], data)
        self.assertEqual(self.request("POST", "/api/deployment/result", {"agent_id": "mind-test", "package_id": "package-1", "success": True})[0], 200)
        self.assertEqual(len(self.fixture.deployment_results), 1)

    def test_unknown_fields_redirect_and_another_certificate(self):
        self.heartbeat()
        self.fixture.inject("/api/commands/poll", "unknown_fields")
        self.assertEqual(self.poll()["future_field"], {"ignored": True})
        self.fixture.inject("/api/config", "redirect")
        status, _, headers = self.request("GET", "/api/config")
        self.assertEqual(status, 307)
        self.assertEqual(headers["Location"], "https://elsewhere.invalid/effector")
        with EffectorFixture() as other:
            self.assertNotEqual(other.leaf_pin, self.fixture.leaf_pin)
            connection = http.client.HTTPSConnection("127.0.0.1", other.port, context=self.context, timeout=3)
            with self.assertRaises(ssl.SSLCertVerificationError):
                connection.request("GET", "/effector/api/config")
            connection.close()

    def test_bounded_bodies_and_strict_json(self):
        for headers in ((('Content-Length', '2'), ('Content-Length', '3')),
                        (('Transfer-Encoding', 'chunked'),)):
            connection = self.connection()
            connection.putrequest("POST", "/effector/api/heartbeat")
            connection.putheader("Authorization", "Bearer " + self.fixture.token)
            connection.putheader("Content-Type", "application/json")
            for key, value in headers:
                connection.putheader(key, value)
            connection.endheaders()
            self.assertEqual(connection.getresponse().status, 400)
            connection.close()
        for body in (b'{"agent_id":"x","agent_id":"y"}', b'{"agent_id":"x","number":NaN}', b"[]"):
            self.assertEqual(self.request("POST", "/api/heartbeat", body, raw=True,
                                          headers={"Content-Type": "application/json"})[0], 400)
        self.assertEqual(self.request("POST", "/api/heartbeat", b" " * (JSON_LIMIT + 1), raw=True)[0], 413)
        with self.assertRaises(ValueError):
            self.fixture.publish(b"x" * (FILE_LIMIT + 1), package_id="large")
        self.assertEqual(self.request("GET", "/api/commands/poll?agent_id=x&agent_id=y")[0], 400)

    def test_telemetry_is_checked_as_the_server_checks_it(self):
        sample = {"schema": 1, "sample_id": "s1792566386-65200", "sampled_at": "2026-10-22T18:06:26Z", "interval_seconds": 30.0,
                  "system": {"logical_cpus": 2, "cpu_percent": 12.5}, "disks": [],
                  "interfaces": [{"name": "card0", "mac_address": "52:54:00:12:34:56", "ip_addresses": ["10.0.2.15"]}],
                  "components": [{"id": "effector", "pids": [38], "cpu_percent": 1.0}]}
        header = {"X-Effector-Agent-ID": "agent-1"}
        self.assertEqual(self.request("POST", "/api/telemetry", sample, headers=header)[0], 404, "registered first")
        self.assertEqual(self.request("POST", "/api/heartbeat", {"agent_id": "agent-1"})[0], 200)
        self.assertEqual(self.request("POST", "/api/telemetry", sample, headers=header)[0], 204)
        self.assertEqual(list(self.fixture.telemetry), [{"agent_id": "agent-1", "sample": sample}])
        for change in ({"schema": 2}, {"interval_seconds": 0}, {"sample_id": "a/b"}, {"system": {"logical_cpus": 0}},
                       {"components": [{"cpu_percent": 100.5}]}, {"interfaces": [{"mac_address": "52-54"}]}):
            self.assertEqual(self.request("POST", "/api/telemetry", {**sample, **change}, headers=header)[0], 400, change)
        self.assertEqual(len(self.fixture.telemetry), 1)


if __name__ == "__main__":
    unittest.main()
