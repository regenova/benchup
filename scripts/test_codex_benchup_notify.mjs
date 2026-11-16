#!/usr/bin/env node
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {createHash, createHmac} from 'node:crypto';
import {mkdtempSync, writeFileSync, rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {CLIENTS, finishServer, notify, readServers} from './codex_benchup_notify.mjs';

const TOKEN = 'notification-test-token';
const proof = (role, nonce, clientNonce) => createHmac('sha256', TOKEN)
  .update(`benchup-agent-link/${role}\n${nonce}\n${clientNonce}`).digest('hex');

async function bridge({badProof = false, silent = false} = {}) {
  const server = createServer(), sockets = new Set(), frames = [];
  server.on('upgrade', (request, socket) => {
    sockets.add(socket); socket.on('close', () => sockets.delete(socket));
    const accept = createHash('sha1').update(request.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    const send = value => {
      const payload = Buffer.from(JSON.stringify(value));
      assert(payload.length < 126);
      socket.write(Buffer.concat([Buffer.from([0x81, payload.length]), payload]));
    };
    let buffered = Buffer.alloc(0);
    socket.on('data', data => {
      buffered = Buffer.concat([buffered, data]);
      while (buffered.length >= 2) {
        const opcode = buffered[0] & 15, masked = (buffered[1] & 128) !== 0;
        let length = buffered[1] & 127, offset = 2;
        if (length === 126) { if (buffered.length < 4) break; length = buffered.readUInt16BE(2); offset = 4; }
        if (buffered.length < offset + (masked ? 4 : 0) + length) break;
        const mask = buffered.subarray(offset, offset + 4); if (masked) offset += 4;
        const payload = Buffer.from(buffered.subarray(offset, offset + length));
        if (masked) for (let i = 0; i < payload.length; i++) payload[i] ^= mask[i % 4];
        buffered = buffered.subarray(offset + length);
        if (opcode === 8) { socket.end(Buffer.from([0x88, 0])); continue; }
        if (opcode !== 1 || silent) continue;
        const frame = JSON.parse(payload.toString()); frames.push(frame);
        if (frame.type === 'authenticate') send({type: 'challenge', protocol: 2, nonce: 'a'.repeat(64)});
        else if (frame.type === 'prove') {
          assert.equal(frame.proof, proof('client', 'a'.repeat(64), frame.nonce));
          send({type: 'ready', proof: badProof ? '0'.repeat(64) : proof('bridge', 'a'.repeat(64), frame.nonce)});
        } else if (frame.type === 'request') send({type: 'response', id: frame.id, ok: true, result: {ok: true}});
      }
    });
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return {port: server.address().port, frames, close: async () => {
    for (const socket of sockets) socket.destroy();
    await new Promise(resolve => server.close(resolve));
  }};
}

test('ignores unrelated events without reading config or contacting BenchUp', async () => {
  assert.deepEqual(await notify({type: 'approval-requested'}, {servers: () => { throw Error('must not read'); }}), []);
});

test('authenticates, verifies the bridge, and sends completion reports only', async () => {
  const peer = await bridge();
  try {
    assert.equal(await finishServer({port: peer.port, token: TOKEN}), CLIENTS.length);
    const requests = peer.frames.filter(frame => frame.type === 'request');
    assert.deepEqual(requests.map(frame => frame.client), CLIENTS);
    assert(CLIENTS.every(client => client.startsWith('codex')));
    assert(requests.every(frame => frame.name === 'agent_activity' && frame.arguments.phase === 'finished'));
    assert(!JSON.stringify(peer.frames).includes(TOKEN), 'never send the raw pairing token');
  } finally { await peer.close(); }
});

test('never sends completion to an unproven bridge', async () => {
  const peer = await bridge({badProof: true});
  try {
    await assert.rejects(finishServer({port: peer.port, token: TOKEN}), /proof failed/);
    assert(!peer.frames.some(frame => frame.type === 'request'));
  } finally { await peer.close(); }
});

test('a missing app or unresponsive bridge cannot hang the notification', async () => {
  const peer = await bridge({silent: true});
  try {
    const started = Date.now();
    const results = await notify({type: 'agent-turn-complete'}, {
      servers: () => [{port: peer.port, token: TOKEN}],
      finish: server => finishServer(server, {timeout: 80}),
    });
    assert.equal(results[0].status, 'rejected');
    assert(Date.now() - started < 1500);
  } finally { await peer.close(); }
});

test('reads only BenchUp connection settings and respects its custom port', () => {
  const root = mkdtempSync(join(tmpdir(), 'benchup-notify-test-'));
  const previous = process.env.CODEX_HOME;
  try {
    process.env.CODEX_HOME = root;
    writeFileSync(join(root, 'config.toml'), `
[mcp_servers.benchup]
command = "benchup"
args = ["bridge", "--port", "48123"]
[mcp_servers.benchup.env]
BENCHUP_AGENT_TOKEN = "fake-test-token"
[mcp_servers.unrelated]
command = "unrelated"
`);
    assert.deepEqual(readServers(), [{port: 48123, token: 'fake-test-token'}]);
  } finally {
    if (previous === undefined) delete process.env.CODEX_HOME; else process.env.CODEX_HOME = previous;
    rmSync(root, {recursive: true, force: true});
  }
});
