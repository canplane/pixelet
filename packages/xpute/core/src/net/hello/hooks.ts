// @xpute/core/net/hello/hooks.ts

import { type Bytes, is_bytes } from "@xpute/spec/lib/kit/array.ts";
import * as encoding from "@xpute/spec/lib/kit/encoding.ts";
import { TLVReader, TLVWriter } from "@xpute/spec/io/marshal/tlv.ts";

import type { XputeManifest } from "@xpute/spec/ipc/topology.ts";
import type { PID, Signature } from "@xpute/spec/net/crypto.ts";
import type { Advert, AdvertByKind, Dial, Handle, PeerByKind, Transports } from "@xpute/spec/net/xport/mod.ts";
import { TransportKind } from "@xpute/spec/net/xport/mod.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";
import type { CentralAdvert } from "@xpute/spec/net/xport/central.ts";
import type { MeshAdvert, MeshIceServer } from "@xpute/spec/net/xport/mesh.ts";

import { PeerDirectory } from "@xpute/core/rpc/peer/directory.ts";

type TLVKeyValue = { tag: number; val: Bytes };

function read_tlv_kv_all(buf: Bytes): TLVKeyValue[] {
  if (!buf.byteLength) return [];
  try {
    const r = new TLVReader(buf);
    const it = r.read_all();
    const out: TLVKeyValue[] = [];
    for (let i = 0; i + 1 < it.length; i += 2) {
      const tag = it[i];
      const val = it[i + 1];
      if (typeof tag !== "number") continue;
      if (!is_bytes(val)) continue;
      out.push({ tag, val });
    }
    return out;
  } catch {
    return [];
  }
}

export type HelloHooks = {
  send_hello_plain: (kind: TransportKind, dst_handle: Handle, rid: RID, plain_hdr: Bytes, sig: Signature) => void;

  encode_advert: <K extends TransportKind>(kind: K) => Bytes;
  decode_advert: <K extends TransportKind>(kind: K, advert_bin: Bytes) => AdvertByKind[K];
  apply_advert: <K extends TransportKind>(handle: Handle, kind: K, advert?: AdvertByKind[K]) => void;

  on_auth_peer: <K extends TransportKind>(kind: K, handle: Handle, peer: PeerByKind[K]) => void;
};

const ADVERT_TAG_CENTRAL_DIAL = 1;
const ADVERT_TAG_MESH_RENDEZVOUS_DIAL = 1;
const ADVERT_TAG_MESH_SIGNAL_DIAL = 2;
const ADVERT_TAG_MESH_ICE_SERVERS = 3;

export function encode_advert_bin(kind: TransportKind, advert?: Advert): Bytes {
  const w = new TLVWriter();

  switch (kind) {
    case TransportKind.CENTRAL: {
      const a = advert as CentralAdvert | undefined;
      if (a?.dial) {
        w.u8(ADVERT_TAG_CENTRAL_DIAL);
        w.bytes(encoding.te.encode(String(a.dial)));
      }
      return w.finish();
    }
    case TransportKind.MESH: {
      const a = (advert as MeshAdvert | undefined) ?? {};
      if (a.rendezvous_dial) {
        w.u8(ADVERT_TAG_MESH_RENDEZVOUS_DIAL);
        w.bytes(encoding.te.encode(String(a.rendezvous_dial)));
      }
      if (a.signal_dial) {
        w.u8(ADVERT_TAG_MESH_SIGNAL_DIAL);
        w.bytes(encoding.te.encode(String(a.signal_dial)));
      }
      if (a.ice_servers && a.ice_servers.length) {
        try {
          w.u8(ADVERT_TAG_MESH_ICE_SERVERS);
          w.bytes(encoding.te.encode(JSON.stringify(a.ice_servers)));
        } catch {}
      }
      return w.finish();
    }
  }
}

export function decode_advert(kind: TransportKind, advert_bin: Bytes): Advert {
  switch (kind) {
    case TransportKind.CENTRAL: {
      let dial: Dial | undefined;
      const kvs = read_tlv_kv_all(advert_bin);
      for (let i = 0; i < kvs.length; i++) {
        const kv = kvs[i];
        if (kv.tag === ADVERT_TAG_CENTRAL_DIAL && !dial) dial = encoding.td.decode(kv.val);
      }
      return { dial };
    }

    case TransportKind.MESH: {
      let rendezvous_dial: Dial | undefined;
      let signal_dial: Dial | undefined;
      let ice_servers: MeshIceServer[] | undefined;

      const kvs = read_tlv_kv_all(advert_bin); // malformed optional tail ignored (compat)
      for (let i = 0; i < kvs.length; i++) {
        const kv = kvs[i];
        switch (kv.tag) {
          case ADVERT_TAG_MESH_RENDEZVOUS_DIAL:
            if (!rendezvous_dial) rendezvous_dial = encoding.td.decode(kv.val);
            break;
          case ADVERT_TAG_MESH_SIGNAL_DIAL:
            if (!signal_dial) signal_dial = encoding.td.decode(kv.val);
            break;
          case ADVERT_TAG_MESH_ICE_SERVERS:
            if (!ice_servers) {
              try {
                const s = encoding.td.decode(kv.val);
                const j = JSON.parse(s);
                if (Array.isArray(j)) {
                  const out: MeshIceServer[] = [];
                  for (const x of j) {
                    if (!x || typeof x !== "object") continue;
                    const urls = x.urls;
                    if (!(typeof urls === "string" || Array.isArray(urls))) continue;
                    const username = x.username;
                    const credential = x.credential;
                    out.push({
                      urls,
                      username: typeof username === "string" ? username : undefined,
                      credential: typeof credential === "string" ? credential : undefined,
                    });
                  }
                  if (out.length) ice_servers = out;
                }
              } catch {}
            }
            break;
          default:
            break;
        }
      }

      return { rendezvous_dial, signal_dial, ice_servers };
    }
  }
}

export function make_hello_hooks(
  xports: Readonly<Transports>,
  dir: PeerDirectory,
  bind_handle_pid: (kind: TransportKind, handle: Handle, pid: PID) => void,
  net_spec: XputeManifest["net"],
): HelloHooks {
  return {
    send_hello_plain: (kind, dst_handle, rid, plain_hdr, sig) => xports[kind].send_hello(dst_handle, rid, plain_hdr, sig),

    encode_advert: <K extends TransportKind>(kind: K): Bytes => encode_advert_bin(kind, net_spec.advert?.[kind]),
    decode_advert: <K extends TransportKind>(kind: K, advert_bin: Bytes): AdvertByKind[K] => decode_advert(kind, advert_bin) as AdvertByKind[K],
    apply_advert: <K extends TransportKind>(handle: Handle, kind: K, advert?: AdvertByKind[K]): void => xports[kind].bind_handle_advert(handle, advert),

    on_auth_peer: <K extends TransportKind>(kind: K, handle: Handle, peer: PeerByKind[K]): void => {
      bind_handle_pid(kind, handle, peer.pid);
      dir.register(peer);
    },
  };
}
