import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const executablePath = process.argv[2];
if (!executablePath) {
  throw new Error("Pass the absolute path to the packaged FloatNote binary.");
}

const executable = resolve(executablePath);
const temporaryRoot = realpathSync(process.env.RUNNER_TEMP || tmpdir());
const scratch = mkdtempSync(join(temporaryRoot, "floatnote-smoke-"));
try {
  const dataDirectory = join(scratch, "data");
  const reportPath = join(dataDirectory, "report.json");
  mkdirSync(dataDirectory);

  const startup = spawnSync(executable, ["--smoke-test"], {
    cwd: scratch,
    env: {
      ...process.env,
      FLOATNOTE_DATA_DIR: dataDirectory,
      FLOATNOTE_SMOKE_REPORT: reportPath,
    },
    stdio: "inherit",
    timeout: 30_000,
    killSignal: "SIGKILL",
  });
  if (startup.error) {
    throw startup.error;
  }
  if (startup.status !== 0) {
    throw new Error(`FloatNote startup failed (exit ${startup.status}, signal ${startup.signal}).`);
  }

  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  if (report.visible !== true || typeof report.shortcutOk !== "boolean") {
    throw new Error(`Unexpected FloatNote startup report ${JSON.stringify(report)}.`);
  }
  console.log(`FloatNote startup passed ${JSON.stringify(report)}.`);
} finally {
  if (dirname(realpathSync(scratch)) === temporaryRoot) {
    rmSync(scratch, { recursive: true, force: true });
  }
}
