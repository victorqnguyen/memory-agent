// memory-agent OpenCode plugin
// Handles session lifecycle and file observation via the OpenCode event system.
//
// Context injection is handled by the memory-agent MCP server in opencode.json.
//
// Installation:
//   memory-agent install opencode
//
// Requires: memory-agent on PATH (or set MEMORY_AGENT_BIN env var)

import type { Plugin } from "@opencode-ai/plugin";

const bin = process.env.MEMORY_AGENT_BIN ?? "memory-agent";

const plugin: Plugin = async ({ project, directory, $ }) => {
  // project.name is optional in the SDK — fall back to directory basename
  const projectName = project.name ?? directory.split("/").pop() ?? "unknown";
  const scope = `/${projectName}`;

  return {
    event: async ({ event }) => {
      switch (event.type) {
        case "session.created": {
          await $`${bin} session-start ${projectName} -d ${directory}`
            .quiet()
            .nothrow();
          break;
        }

        case "file.edited": {
          const { file } = event.properties;
          await $`${bin} observe-edit ${file} --scope ${scope}`
            .quiet()
            .nothrow();
          break;
        }

        case "session.compacted": {
          // Extract learnings from project config files after compaction
          await $`${bin} extract -d ${directory} --scope ${scope}`
            .quiet()
            .nothrow();
          break;
        }

        case "session.deleted": {
          const { info } = event.properties;
          await $`${bin} session-end ${info.id} --summary "OpenCode session ended"`
            .quiet()
            .nothrow();
          break;
        }
      }
    },
  };
};

export default plugin;
