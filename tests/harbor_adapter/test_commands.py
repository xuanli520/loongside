from __future__ import annotations

import subprocess
import tomllib
import unittest

from harbor_loongclaw.commands import build_agent_install_command
from harbor_loongclaw.commands import build_agent_run_command
from harbor_loongclaw.commands import build_runtime_config_text


def assert_shell_parses(script: str) -> None:
    completed = subprocess.run(
        ["bash", "-n"],
        check=False,
        input=script,
        text=True,
        capture_output=True,
    )

    if completed.returncode == 0:
        return

    error_message = completed.stderr.strip()
    raise AssertionError(error_message)


class HarborLoongClawCommandTests(unittest.TestCase):
    def test_install_command_is_valid_shell(self) -> None:
        command = build_agent_install_command("/opt/loongclaw-src")
        assert_shell_parses(command)

    def test_install_command_uses_cargo_install_without_source_fallback(self) -> None:
        command = build_agent_install_command("/opt/loongclaw-src")

        self.assertIn("--bin loong", command)
        self.assertIn("--bin loongclaw", command)
        self.assertNotIn("scripts/install.sh", command)

    def test_runtime_config_text_is_valid_toml(self) -> None:
        config_text = build_runtime_config_text(
            profile_id="openai",
            provider_kind="openai",
            model_id="gpt-5.4",
            reasoning_effort="xhigh",
            api_key_env="OPENAI_API_KEY",
            shell_default_mode="allow",
        )

        parsed = tomllib.loads(config_text)

        provider = parsed["providers"]["openai"]
        tools = parsed["tools"]
        bash_tools = tools["bash"]

        self.assertEqual(parsed["active_provider"], "openai")
        self.assertEqual(provider["kind"], "openai")
        self.assertEqual(provider["model"], "gpt-5.4")
        self.assertEqual(provider["reasoning_effort"], "xhigh")
        self.assertEqual(provider["api_key"]["env"], "OPENAI_API_KEY")
        self.assertEqual(tools["file_root"], "$TASK_CWD")
        self.assertEqual(tools["shell_default_mode"], "allow")
        self.assertFalse(bash_tools["login_shell"])

    def test_run_command_is_valid_shell(self) -> None:
        command = build_agent_run_command(
            profile_id="openai",
            provider_kind="openai",
            model_id="gpt-5.4",
            reasoning_effort="xhigh",
            api_key_env="OPENAI_API_KEY",
            shell_default_mode="allow",
            session_name="harbor",
            instruction="say hello",
            config_path="/logs/agent/loongclaw-config.toml",
            output_path="/logs/agent/loongclaw.txt",
            trajectory_path="/logs/agent/loongclaw-trajectory.json",
        )

        assert_shell_parses(command)

    def test_run_command_keeps_validate_ask_and_trajectory_contract(self) -> None:
        command = build_agent_run_command(
            profile_id="openai",
            provider_kind="openai",
            model_id="gpt-5.4",
            reasoning_effort="xhigh",
            api_key_env="OPENAI_API_KEY",
            shell_default_mode="allow",
            session_name="harbor",
            instruction="say hello",
            config_path="/logs/agent/loongclaw-config.toml",
            output_path="/logs/agent/loongclaw.txt",
            trajectory_path="/logs/agent/loongclaw-trajectory.json",
        )

        self.assertIn("loong validate-config", command)
        self.assertIn("loong ask", command)
        self.assertIn("loong trajectory-export", command)


if __name__ == "__main__":
    unittest.main()
