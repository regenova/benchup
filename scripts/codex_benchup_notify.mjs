#!/usr/bin/env node
// Codex `notify` hook: finish live BenchUp activity on agent-turn-complete.
// Requires Node 22+ and Python 3.11+; no builds or third-party packages.
import {createHmac, randomBytes, timingSafeEqual} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {resolve} from 'node:path';

// Codex versions use different MCP client names. Leave generic shell clients
// and other hosts (e.g. Claude) alone; they may still be working.
export const CLIENTS = ['codex', 'codex-cli', 'codex-mcp-client'];

export function readServers() {
  // Parse TOML with the standard library. Pairing secrets stay in this pipe
  // and memory; they are never placed in argv, logs, or the hook config.
  const source = `
import json, os, pathlib, tomllib
path = pathlib.Path(os.environ.get('CODEX_HOME', str(pathlib.Path.home()/'.codex')))/'config.toml'
data = tomllib.loads(path.read_text()) if path.exists() else {}
servers = []
for server in data.get('mcp_servers', {}).values():
    command = pathlib.Path(server.get('command', '')).name.lower()
    if command not in ('benchup', 'benchup.exe'): continue
    env = server.get('env', {})
    token = env.get('BENCHUP_AGENT_TOKEN', os.environ.get('BENCHUP_AGENT_TOKEN'))
    port = env.get('BENCHUP_BRIDGE_PORT', os.environ.get('BENCHUP_BRIDGE_PORT', '47615'))
    args = server.get('args', [])
    if '--port' in args: port = args[args.index('--port')+1]
    if token: servers.append({'token': token, 'port': int(port)})
if not servers and os.environ.get('BENCHUP_AGENT_TOKEN'):
    servers.append({'token': os.environ['BENCHUP_AGENT_TOKEN'], 'port': int(os.environ.get('BENCHUP_BRIDGE_PORT', '47615'))})
print(json.dumps(servers))
`;
  return JSON.parse(execFileSync('python3', ['-c', source], {encoding: 'utf8', timeout: 2000, stdio: ['ignore', 'pipe', 'pipe']}));
}

function proof(role, token, bridgeNonce, clientNonce) {
  return createHmac('sha256', token).update(`benchup-agent-link/${role}\n${bridgeNonce}\n${clientNonce}`).digest('hex');
}

export async function finishServer({port, token}, {clients = CLIENTS, timeout = 6000} = {}) {
  if (!Number.isInteger(port) || port < 1024 || port > 65535 || typeof token !== 'string' || !token) {
    throw Error('Invalid BenchUp connection');
  }
  const socket = new WebSocket(`ws://127.0.0.1:${port}/`);
  const messages = [], waiters = [];
  let failure;
  const fail = () => {
    failure = Error('BenchUp notification connection ended');
    while (waiters.length) waiters.shift().reject(failure);
  };
  socket.addEventListener('error', fail);
  socket.addEventListener('close', fail);
  socket.addEventListener('message', event => {
    try {
      if (typeof event.data !== 'string' || event.data.length > 65536) throw Error();
      const message = JSON.parse(event.data);
      if (waiters.length) waiters.shift().resolve(message);
      else messages.push(message);
    } catch { fail(); socket.close(); }
  });
  const receive = () => failure ? Promise.reject(failure) : messages.length
    ? Promise.resolve(messages.shift())
    : new Promise((resolve, reject) => waiters.push({resolve, reject}));
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => { fail(); socket.close(); reject(Error('BenchUp notification timed out')); }, timeout);
  });
  try {
    return await Promise.race([deadline, (async () => {
      await new Promise((resolve, reject) => {
        socket.addEventListener('open', resolve, {once: true});
        socket.addEventListener('error', () => reject(Error('BenchUp bridge is unavailable')), {once: true});
      });
      const send = value => socket.send(JSON.stringify(value));
      send({type: 'authenticate', role: 'client', connection: 'benchup-codex-notify'});
      const challenge = await receive();
      if (challenge.type !== 'challenge' || challenge.protocol !== 2 || !/^[0-9a-f]{64}$/.test(challenge.nonce)) throw Error('Incompatible BenchUp bridge');
      const nonce = randomBytes(32).toString('hex');
      send({type: 'prove', nonce, proof: proof('client', token, challenge.nonce, nonce)});
      const ready = await receive();
      const expected = proof('bridge', token, challenge.nonce, nonce);
      if (ready.type !== 'ready' || typeof ready.proof !== 'string' || ready.proof.length !== expected.length ||
          !timingSafeEqual(Buffer.from(ready.proof), Buffer.from(expected))) throw Error('BenchUp bridge proof failed');
      let finished = 0;
      for (const [index, client] of clients.entries()) {
        send({type: 'request', id: index + 1, name: 'agent_activity', arguments: {phase: 'finished'}, client});
        const answer = await receive();
        if (answer.type !== 'response' || answer.id !== index + 1 || answer.ok !== true) throw Error('BenchUp did not accept completion');
        finished++;
      }
      return finished;
    })()]);
  } finally {
    clearTimeout(timer);
    socket.close();
  }
}

export async function notify(event, {servers = readServers, finish = finishServer} = {}) {
  if (!event || event.type !== 'agent-turn-complete') return [];
  // An unavailable/closed app is normal. Do not launch a bridge or app just
  // because Codex completed an unrelated turn, and never leak payloads/errors.
  return Promise.allSettled(servers().map(server => finish(server)));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { await notify(JSON.parse(process.argv[2] ?? '{}')); }
  catch { /* Notification hooks must not interfere with Codex's final reply. */ }
  process.exit(0);
}
