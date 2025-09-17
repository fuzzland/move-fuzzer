#!/usr/bin/env python3

import time
import signal
import sys
import os
import subprocess
import shlex
import threading
from datetime import datetime
from pathlib import Path

import toml
import requests
import psutil
from rich.console import Console
from rich.table import Table
from rich import print as rprint
from jinja2 import Template

console = Console()

class IntegrationTester:
    def __init__(self):
        self.script_dir = Path(__file__).parent
        self.root_dir = self.script_dir.parent
        self.sui_binary = self.script_dir / "sui"
        self.localnet_process = None
        self.package_id = None
        self.shared_object_id = None
        self.test_results = []

        # Load test configuration
        config_file = self.script_dir / "test_cases.toml"
        with open(config_file, 'r') as f:
            self.config = toml.load(f)

    def log(self, message, level="INFO"):
        """Enhanced logging with rich console"""
        timestamp = datetime.now().strftime("%H:%M:%S")
        colors = {
            "INFO": "[white]",
            "SUCCESS": "[green]",
            "WARNING": "[yellow]",
            "ERROR": "[red]",
            "DEBUG": "[cyan]"
        }
        color = colors.get(level, "[white]")
        console.print(f"{color}[{timestamp}] {message}[/]")

    def run_sui_command(self, *args, cwd=None, timeout=30, ignore_error=False):
        """Helper to run sui commands using subprocess"""
        cmd = [str(self.sui_binary)] + list(args)
        try:
            result = subprocess.run(
                cmd,
                cwd=cwd or self.script_dir,
                capture_output=True,
                text=True,
                timeout=timeout
            )
            if result.returncode != 0 and not ignore_error:
                self.log(f"Sui command failed: {' '.join(args)}", "ERROR")
                if result.stderr:
                    self.log(f"Error: {result.stderr.strip()}", "ERROR")
                if result.stdout:
                    self.log(f"Output: {result.stdout.strip()}", "ERROR")
                raise Exception(f"Command failed with code {result.returncode}")
            return result
        except subprocess.TimeoutExpired:
            self.log(f"Sui command timed out: {' '.join(args)}", "ERROR")
            raise Exception("Command timed out")

    def is_port_open(self, port, timeout=30):
        """Check if port is open using psutil with fallback to socket"""
        self.log(f"Checking port {port}...")
        for i in range(timeout):
            # Method 1: Try psutil (may have permission issues)
            try:
                for conn in psutil.net_connections():
                    if hasattr(conn, 'laddr') and conn.laddr and conn.laddr.port == port:
                        self.log(f"Port {port} is open (psutil)", "SUCCESS")
                        return True
            except (psutil.AccessDenied, psutil.NoSuchProcess) as e:
                self.log(f"psutil permission issue: {e}", "DEBUG")
            except Exception as e:
                self.log(f"psutil error: {e}", "DEBUG")

            # Method 2: Try socket connection (more reliable)
            try:
                import socket
                sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                sock.settimeout(1)
                result = sock.connect_ex(('localhost', port))
                sock.close()
                if result == 0:
                    self.log(f"Port {port} is open (socket)", "SUCCESS")
                    return True
            except Exception as e:
                self.log(f"socket error: {e}", "DEBUG")

            if i < timeout - 1:  # Don't sleep on last iteration
                time.sleep(1)

        self.log(f"Port {port} is not open after {timeout}s", "WARNING")
        return False

    def kill_process_on_port(self, port):
        """Kill any process using the given port"""
        for proc in psutil.process_iter(['pid', 'name']):
            try:
                for conn in proc.net_connections():
                    if conn.laddr.port == port:
                        self.log(f"Killing process {proc.info['name']} (PID: {proc.info['pid']}) on port {port}")
                        proc.kill()
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass

    def build_fuzzer(self):
        """Build move-fuzzer in release mode"""
        self.log("Building move-fuzzer...")
        try:
            result = subprocess.run(
                ["cargo", "build", "--release"],
                cwd=self.root_dir,
                timeout=3600
            )
            if result.returncode != 0:
                self.log("Build failed:", "ERROR")
                return False

            self.log("Build completed successfully", "SUCCESS")
            return True
        except subprocess.TimeoutExpired:
            self.log("Build timed out", "ERROR")
            return False
        except Exception as e:
            self.log(f"Build failed: {e}", "ERROR")
            return False

    def cleanup_localnet_process(self):
        """Force cleanup localnet process to avoid hanging"""
        if self.localnet_process:
            try:
                self.log("Terminating localnet process...")
                self.localnet_process.terminate()

                # Wait for graceful termination
                try:
                    self.localnet_process.wait(timeout=3)
                    self.log("Localnet process terminated gracefully", "SUCCESS")
                except subprocess.TimeoutExpired:
                    self.log("Force killing localnet process...")
                    self.localnet_process.kill()
                    try:
                        self.localnet_process.wait(timeout=2)
                        self.log("Localnet process killed", "SUCCESS")
                    except subprocess.TimeoutExpired:
                        self.log("Process may still be running", "WARNING")

            except Exception as e:
                self.log(f"Error during cleanup: {e}", "DEBUG")
            finally:
                self.localnet_process = None

    def start_localnet(self):
        """Start localnet in background and verify both ports"""
        self.log("Starting localnet...")

        # Kill any existing processes on required ports
        self.kill_process_on_port(9000)
        self.kill_process_on_port(9123)
        time.sleep(2)

        # Start localnet in background using subprocess
        try:
            cmd = [
                str(self.sui_binary), "start",
                "--with-faucet",
                "--force-regenesis"
            ]

            env = os.environ.copy()
            env["RUST_LOG"] = "off,sui_node=info"

            # Use subprocess.Popen for better control
            self.localnet_process = subprocess.Popen(
                cmd,
                cwd=str(self.script_dir),
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,  # Combine stderr with stdout
                text=True,
                bufsize=1  # Line buffered
            )

            # Wait a moment to check if process started successfully
            self.log("Checking if localnet process started...")
            time.sleep(2)

            # Check if process is still running
            poll_result = self.localnet_process.poll()
            if poll_result is not None:
                # Process died, capture output
                self.log(f"Localnet process exited early with code: {poll_result}", "ERROR")
                try:
                    output, _ = self.localnet_process.communicate(timeout=1)
                    self.log("Localnet startup output:", "ERROR")
                    for line in output.splitlines()[-20:]:  # Last 20 lines
                        self.log(f"  {line}", "ERROR")
                except subprocess.TimeoutExpired:
                    self.log("Could not capture process output", "WARNING")

                self.cleanup_localnet_process()
                return False

            # Process is running, continue with longer wait
            self.log("Localnet process started successfully, waiting 6 seconds for initialization...")

            # Start output monitoring thread
            def monitor_localnet_output():
                """Monitor localnet output in background"""
                try:
                    while self.localnet_process and self.localnet_process.poll() is None:
                        line = self.localnet_process.stdout.readline()
                        if line:
                            line = line.strip()
                            # Show important messages
                            if any(keyword in line.lower() for keyword in ['error', 'fatal', 'panic', 'failed', 'listening']):
                                self.log(f"[localnet] {line}", "DEBUG")
                        else:
                            break
                except Exception as e:
                    self.log(f"Output monitor error: {e}", "DEBUG")

            # Start monitoring thread
            monitor_thread = threading.Thread(target=monitor_localnet_output, daemon=True)
            monitor_thread.start()

            time.sleep(6)

            # Check both RPC (9000) and faucet (9123) ports with extended timeout
            self.log("Checking if ports 9000 and 9123 are available...")

            # Check if process is still alive before checking ports
            poll_result = self.localnet_process.poll()
            if poll_result is not None:
                self.log(f"Localnet process died during startup with code: {poll_result}", "ERROR")
                try:
                    output, _ = self.localnet_process.communicate(timeout=2)
                    self.log("Localnet process output:", "ERROR")
                    for line in output.splitlines()[-30:]:  # Last 30 lines
                        self.log(f"  {line}", "ERROR")
                except subprocess.TimeoutExpired:
                    self.log("Could not capture process output", "WARNING")
                except Exception:
                    pass
                self.cleanup_localnet_process()
                return False

            # Check ports with better error handling
            try:
                port_9000_open = self.is_port_open(9000, timeout=30)
                port_9123_open = self.is_port_open(9123, timeout=15)

                if port_9000_open and port_9123_open:
                    self.log("Localnet started successfully", "SUCCESS")
                    time.sleep(3)  # Extra stability wait
                    return True
                else:
                    self.log("Ports not available after waiting:", "ERROR")
                    self.log(f"  Port 9000 (RPC): {'OPEN' if port_9000_open else 'CLOSED'}", "ERROR")
                    self.log(f"  Port 9123 (Faucet): {'OPEN' if port_9123_open else 'CLOSED'}", "ERROR")

                    # Check if process is still alive
                    if self.localnet_process.poll() is None:
                        self.log("Process is still running but ports are not open", "ERROR")
                        # Try to get some output for diagnosis
                        try:
                            # Give process a moment to produce output
                            time.sleep(1)
                            # Non-blocking read attempt
                            import select
                            if hasattr(select, 'select'):
                                ready, _, _ = select.select([self.localnet_process.stdout], [], [], 0)
                                if ready:
                                    line = self.localnet_process.stdout.readline()
                                    if line:
                                        self.log(f"Recent output: {line.strip()}", "DEBUG")
                        except Exception:
                            pass
                    else:
                        self.log("Process has died", "ERROR")

                    self.cleanup_localnet_process()
                    return False

            except Exception as port_error:
                self.log(f"Error during port checking: {port_error}", "ERROR")
                self.cleanup_localnet_process()
                return False

        except Exception as e:
            self.log(f"Failed to start localnet: {e}", "ERROR")
            self.cleanup_localnet_process()
            return False

    def get_active_address(self):
        """Get current active address"""
        try:
            result = self.run_sui_command("client", "active-address")
            address = result.stdout.strip()
            self.log(f"Active address: {address}", "DEBUG")
            return address
        except Exception as e:
            self.log(f"Failed to get active address: {e}", "ERROR")
            return None

    def request_faucet_via_http(self, address):
        """Request faucet via HTTP API"""
        try:
            self.log("Requesting faucet via HTTP API...")
            response = requests.post(
                "http://localhost:9123/gas",
                json={"FixedAmountRequest": {"recipient": address}},
                timeout=10
            )

            if response.status_code == 200:
                data = response.json()
                success = data.get("status") == "Success"
                if success:
                    coins_count = len(data.get("coins_sent", []))
                    self.log(f"Faucet HTTP request successful: {coins_count} coins sent", "SUCCESS")
                else:
                    self.log(f"Faucet HTTP request failed: {data}", "ERROR")
                return success
            else:
                self.log(f"Faucet HTTP request failed: {response.status_code} {response.text}", "ERROR")
                return False

        except Exception as e:
            self.log(f"Faucet HTTP request error: {e}", "ERROR")
            return False

    def setup_wallet(self):
        """Setup wallet with improved faucet handling"""
        self.log("Setting up wallet...")

        try:
            # Try to create new address (ignore failure if alias exists)
            try:
                self.run_sui_command("client", "new-address", "ed25519", "move-fuzzer")
            except Exception:
                pass  # Ignore if alias already exists

            # Switch to address (this must succeed)
            self.run_sui_command("client", "switch", "--address", "move-fuzzer")

            # Try to create new env (ignore failure if alias exists)
            try:
                self.run_sui_command("client", "new-env", "--alias", "local", "--rpc", "http://127.0.0.1:9000")
            except Exception:
                pass  # Ignore if alias already exists

            # Switch to env (this must succeed)
            self.run_sui_command("client", "switch", "--env", "local")

            # Get current address for faucet
            address = self.get_active_address()
            if not address:
                return False

            # Try standard faucet command first
            faucet_success = False
            try:
                self.log("Trying standard faucet command...")
                result = self.run_sui_command("client", "faucet")
                self.log(f"Faucet command output: {result.stdout}", "DEBUG")
                faucet_success = True
            except Exception as e:
                self.log(f"Standard faucet failed: {e}", "WARNING")
                # Try HTTP API as fallback
                faucet_success = self.request_faucet_via_http(address)

            if not faucet_success:
                self.log("Both faucet methods failed", "ERROR")
                return False

            # Wait for processing
            time.sleep(3)

            # Verify gas is available
            try:
                gas_result = self.run_sui_command("client", "gas")
                gas_output = gas_result.stdout.strip()
                self.log(f"Gas check output: {gas_output}", "DEBUG")

                if not gas_output or "No gas coins are owned" in gas_output:
                    self.log("No gas objects available after faucet", "ERROR")
                    return False

                self.log("Wallet setup completed successfully", "SUCCESS")
                return True

            except Exception as e:
                self.log(f"Gas check failed: {e}", "ERROR")
                return False

        except Exception as e:
            self.log(f"Wallet setup failed: {e}", "ERROR")
            return False

    def deploy_contract(self):
        """Deploy shl_demo contract and capture package ID"""
        self.log("Deploying shl_demo contract...")

        try:
            contract_dir = self.root_dir / "contracts" / "sui-demo"

            # Build contract
            self.run_sui_command("move", "build", cwd=contract_dir)
            self.log("Contract built successfully", "SUCCESS")

            # Deploy contract
            result = self.run_sui_command("client", "publish", "--gas-budget", "100000000", cwd=contract_dir)
            output = result.stdout

            # Extract package ID
            import re
            # Match "PackageID: 0x..." format from publish output
            package_pattern = r"PackageID:\s*(0x[a-fA-F0-9]+)"
            match = re.search(package_pattern, output, re.IGNORECASE)
            if match:
                self.package_id = match.group(1)
                self.log(f"Contract deployed successfully: {self.package_id}", "SUCCESS")
                return True
            else:
                self.log("Failed to extract package ID from deployment output", "ERROR")
                self.log("Output snippet (first 500 chars):", "DEBUG")
                self.log(output[:500], "DEBUG")
                self.log("Full output:", "DEBUG")
                self.log(output, "DEBUG")
                return False

        except Exception as e:
            self.log(f"Contract deployment failed: {e}", "ERROR")
            return False

    def create_test_objects(self):
        """Create shared objects for testing"""
        self.log("Creating test objects...")

        try:
            # Create shared demo struct
            result = self.run_sui_command(
                "client", "call",
                "--package", self.package_id,
                "--module", "shl_demo",
                "--function", "create_shared_demo_struct",
                "--args", "12", "2"
            )

            # Log full output for debugging
            self.log("Create shared demo struct output:", "DEBUG")
            self.log(result.stdout, "DEBUG")

            # Extract object ID from Created Objects section
            import re

            # Find the Created Objects section and get the first ObjectID after it
            created_pattern = r"Created Objects:.*?ObjectID:\s*(0x[a-fA-F0-9]+)"
            match = re.search(created_pattern, result.stdout, re.DOTALL | re.IGNORECASE)

            if match:
                self.shared_object_id = match.group(1)
                self.log(f"Extracted shared object ID: {self.shared_object_id}", "DEBUG")
            else:
                # Fallback: try simpler pattern and take first match
                fallback_pattern = r"│\s*ObjectID:\s*(0x[a-fA-F0-9]+)"
                matches = re.findall(fallback_pattern, result.stdout, re.IGNORECASE)
                if matches:
                    self.shared_object_id = matches[0]  # Take first match
                    self.log(f"Extracted shared object ID (fallback): {self.shared_object_id}", "DEBUG")
                else:
                    self.log("Could not extract shared object ID from output", "ERROR")
                    self.log("Output snippet (first 1000 chars):", "DEBUG")
                    self.log(result.stdout[:1000], "DEBUG")

            self.log(f"Created shared object: {self.shared_object_id}", "SUCCESS")
            return True

        except Exception as e:
            self.log(f"Failed to create test objects: {e}", "ERROR")
            return False

    def run_fuzzer_test(self, test_case):
        """Run a single fuzzer test case"""
        name = test_case["name"]
        function = test_case["function"]
        iterations = test_case.get("iterations", 100000)
        timeout = test_case.get("timeout", 30)

        # Prepare arguments
        args = test_case.get("args", "")
        if function == "mutable_shared_struct_shl":
            args = self.shared_object_id

        self.log(f"Running test: {name} ({iterations:,} iterations)")
        start_time = time.time()

        try:
            # Build fuzzer command arguments
            fuzzer_cmd = ["cargo", "run", "-p", "fuzzer", "--release", "--", "sui"]
            fuzzer_args = [
                "--rpc-url", "http://localhost:9000",
                "--package", self.package_id,
                "--module", "shl_demo",
                "--function", function
            ]

            # Add arguments if present
            if args:
                fuzzer_args.append("--args")
                # Handle different argument formats
                if args.startswith('[') and args.endswith(']'):
                    # Vector argument - keep as single argument
                    fuzzer_args.append(args)
                else:
                    # Split space-separated arguments
                    fuzzer_args.extend(shlex.split(args))

            fuzzer_args.extend(["--iterations", str(iterations)])

            # Add type arguments if specified
            if "type_args" in test_case:
                type_args = test_case["type_args"]
                fuzzer_args.append("--type-args")
                fuzzer_args.extend(shlex.split(type_args))

            # Combine command and arguments
            full_cmd = fuzzer_cmd + fuzzer_args

            # Set up environment
            env = os.environ.copy()
            env["RUST_LOG"] = "info"

            # Run with real-time output monitoring
            self.log(f"Starting fuzzer with command: {' '.join(full_cmd)}", "DEBUG")

            process = subprocess.Popen(
                full_cmd,
                cwd=self.root_dir,
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1  # Line buffered
            )

            output_lines = []
            violations_found = False

            try:
                # Monitor output in real-time
                while True:
                    line = process.stdout.readline()
                    if not line:
                        break

                    line = line.strip()
                    if line:
                        output_lines.append(line)

                        # Show important lines
                        if any(keyword in line.lower() for keyword in
                               ['iteration', 'shift', 'violation', 'detected', 'error', 'failed']):
                            self.log(f"[fuzzer] {line}", "DEBUG")

                        # Always show progress every 10000 iterations
                        if "iteration" in line.lower() and ("10000" in line or "000000" in line):
                            self.log(f"[fuzzer] {line}", "INFO")

                        # Check for violations
                        if "SHIFT VIOLATION DETECTED" in line.upper():
                            violations_found = True
                            self.log(f"[fuzzer] VIOLATION FOUND: {line}", "SUCCESS")

                # Wait for process completion
                return_code = process.wait(timeout=timeout)
                end_time = time.time()
                execution_time = end_time - start_time

                success = (return_code == 0) or violations_found  # Success if clean exit or violation found

            except subprocess.TimeoutExpired:
                self.log(f"Test {name} timed out after {timeout}s, terminating...", "WARNING")
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()

                end_time = time.time()
                execution_time = end_time - start_time
                success = False
                violations_found = False

        except Exception as e:
            end_time = time.time()
            execution_time = end_time - start_time
            success = False
            violations_found = False
            self.log(f"Test {name} failed: {e}", "ERROR")

        # Record result
        test_result = {
            "name": name,
            "function": function,
            "success": success,
            "violations_found": violations_found,
            "expected_violations": test_case.get("expected_violations", False),
            "execution_time": execution_time,
            "iterations": iterations
        }

        self.test_results.append(test_result)

        # Log result
        if success:
            if violations_found:
                self.log(f"Test {name}: VIOLATIONS FOUND ✓", "SUCCESS")
            else:
                self.log(f"Test {name}: NO VIOLATIONS", "WARNING")
        else:
            self.log(f"Test {name}: FAILED ✗", "ERROR")

        return success

    def run_all_tests(self):
        """Run all test cases"""
        self.log("Running fuzzer tests...")

        for test_case in self.config["test_cases"]:
            self.run_fuzzer_test(test_case)
            time.sleep(1)  # Brief pause between tests

        return True

    def generate_report(self):
        """Generate test report with rich formatting"""
        self.log("Generating test report...")

        # Create rich table for console output
        table = Table(title="Move Fuzzer Integration Test Results")
        table.add_column("Test", style="cyan")
        table.add_column("Passed", justify="center", style="bold")
        table.add_column("Time", justify="right")

        all_passed = True

        for result in self.test_results:
            expected = result["expected_violations"]
            found = result["violations_found"]
            passed = result["success"] and (expected == found)
            all_passed = all_passed and passed

            # Add row to table
            table.add_row(
                result["name"],
                "[green]✓[/green]" if passed else "[red]✗[/red]",
                f"{result['execution_time']:.1f}s"
            )

        # Display table
        console.print()
        console.print(table)

        # Generate Markdown report using Jinja2
        report_template = Template("""## SUI Fuzzer Integration Test Report

**Generated**: {{ timestamp }}
**Package ID**: {{ package_id }}
**Overall Result**: {{ "PASSED" if all_passed else "FAILED" }}

### Test Results

| Test | Passed | Time |
|------|--------|------|
{%- for result in results %}
| {{ result.name }} | {{ "✓" if result.passed else "✗" }} | {{ "%.1f"|format(result.execution_time) }}s |
{%- endfor %}
""")

        # Prepare template data
        results_with_passed = []
        for result in self.test_results:
            result_copy = result.copy()
            result_copy['passed'] = result['success'] and (result['expected_violations'] == result['violations_found'])
            results_with_passed.append(result_copy)

        report_content = report_template.render(
            timestamp=datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            package_id=self.package_id,
            all_passed=all_passed,
            results=results_with_passed
        )

        # Write report file
        report_file = self.root_dir / "test-report.md"
        with open(report_file, 'w') as f:
            f.write(report_content)

        console.print(f"\n📄 Report saved to: [cyan]{report_file}[/cyan]")

        if all_passed:
            self.log("All tests PASSED! 🎉", "SUCCESS")
            return True
        else:
            self.log("Some tests FAILED ❌", "ERROR")
            return False

    def cleanup(self):
        """Clean up background processes"""
        # Use the improved cleanup function
        self.cleanup_localnet_process()

        # Kill any remaining processes on required ports
        self.kill_process_on_port(9000)
        self.kill_process_on_port(9123)

    def run(self):
        """Run the complete integration test"""
        try:
            rprint("[bold blue]🚀 Starting Move Fuzzer Integration Tests[/bold blue]")

            if not self.build_fuzzer():
                return False

            if not self.start_localnet():
                # Ensure cleanup on localnet startup failure
                self.cleanup_localnet_process()
                return False

            if not self.setup_wallet():
                return False

            if not self.deploy_contract():
                return False

            if not self.create_test_objects():
                return False

            if not self.run_all_tests():
                return False

            return self.generate_report()

        except KeyboardInterrupt:
            self.log("Tests interrupted by user", "WARNING")
            return False
        except Exception as e:
            self.log(f"Unexpected error: {e}", "ERROR")
            return False
        finally:
            self.cleanup()

def main():
    tester = IntegrationTester()

    # Handle Ctrl+C gracefully
    def signal_handler(_, __):
        tester.cleanup()
        sys.exit(1)

    signal.signal(signal.SIGINT, signal_handler)

    # Run tests
    success = tester.run()
    sys.exit(0 if success else 1)

if __name__ == "__main__":
    main()