// // @pxl/api/xpute/container.ts

// type Ctor<T> = new (...args: any[]) => T;

// // Token: class-ctor or symbol (HOST_LINK 같은 값 토큰)
// export type Token<T> = Ctor<T> | symbol;

// // deps metadata slot (no reflect-metadata)
// export const DEPS: unique symbol = Symbol("xpute.deps");

// export const IMPL: unique symbol = Symbol("xpute.impl");

// // class decorator: store deps in a symbol slot (no runtime reflection)
// export function deps(...tokens: Token<any>[]) {
//   return function <T extends Function>(ctor: T): void {
//     (ctor as any)[DEPS] = tokens;
//   };
// }

// // class decorator: register ctor as hidden impl of an export token (no reflection)
// export function impl<T>(token: Token<T>) {
//   return function <T extends Function>(ctor: T): void {
//     (token as any)[IMPL] = ctor as any;
//   };
// }

// type Provider<Ctx> =
//   | { kind: "value"; value: any }
//   | { kind: "instance"; instance: any }
//   | { kind: "class"; ctor: Ctor<any> }
//   | { kind: "factory"; make: (app: AppContainer<Ctx>) => any }
//   | { kind: "alias"; to: Token<any> };

// // -------------------------
// // module layer
// // -------------------------

// export type AppModule<Ctx> = {
//   readonly imports?: readonly AppModule<Ctx>[];
//   readonly exports?: readonly Token<any>[];
//   bind(app: AppContainer<Ctx>): void;
// };

// export class ExportedContainer<Ctx> {
//   constructor(
//     private readonly app: AppContainer<Ctx>,
//     private readonly exports: ReadonlySet<Token<any>>,
//   ) {}

//   has<T>(key: Token<T>): boolean {
//     if (!this.exports.has(key)) return false;
//     return this.app.has(key);
//   }

//   get<T>(ctor: Ctor<T>): T {
//     if (!this.exports.has(ctor as any)) throw new Error("exported.get: token not exported");
//     return this.app.get(ctor);
//   }

//   get_value<T>(key: Token<T>): T {
//     if (!this.exports.has(key)) throw new Error("exported.get_value: token not exported");
//     return this.app.get_value(key);
//   }

//   get_optional<T>(key: Token<T>, fallback: T | null = null): T | null {
//     if (!this.exports.has(key)) return fallback;
//     return this.app.get_optional(key, fallback);
//   }
// }

// export interface Lifecycle<Ctx> {
//   mount(ctx: Ctx): void | Promise<void>;
//   unmount(): void | Promise<void>;
// }

// type MaybeLifecycle<Ctx> = Partial<Lifecycle<Ctx>>;

// export class AppContainer<Ctx> {
//   private readonly providers = new Map<Token<any>, Provider<Ctx>>();
//   private readonly singletons = new Map<Token<any>, any>();
//   private readonly building = new Set<Token<any>>();

//   private readonly create_order: Token<any>[] = [];
//   private readonly created = new Set<Token<any>>();

//   private readonly installed_modules = new Set<AppModule<Ctx>>();
//   private readonly export_set = new Set<Token<any>>();

//   private on_error: ((err: unknown, where: "mount" | "unmount", key: Token<any>) => void) | null = null;

//   set_error_hook(cb: ((err: unknown, where: "mount" | "unmount", key: Token<any>) => void) | null): void {
//     this.on_error = cb;
//   }

//   // NEW: install module graph (imports-first)
//   install(mod: AppModule<Ctx>): void {
//     if (this.installed_modules.has(mod)) return;
//     this.installed_modules.add(mod);

//     const imports = mod.imports ?? [];
//     for (const im of imports) this.install(im);

//     mod.bind(this);

//     const ex = mod.exports ?? [];
//     for (const t of ex) this.export_set.add(t);
//   }

//   // NEW: exported view (enforce boundary)
//   exported(): ExportedContainer<Ctx> {
//     return new ExportedContainer<Ctx>(this, this.export_set);
//   }

//   // optional macro: bind a bunch of providers in one place (Nest module 느낌)
//   bind_package(bind: (app: AppContainer<Ctx>) => void): void {
//     bind(this);
//   }

//   bind_value<T>(key: Token<T>, value: T): void {
//     this.providers.set(key, { kind: "value", value });
//   }

//   bind_instance<T>(key: Token<T>, instance: T): void {
//     this.providers.set(key, { kind: "instance", instance });
//   }

//   // bind token -> class
//   // - bind_class(Foo)          : token==impl
//   // - bind_class(Foo, LocalFoo): token==Foo, impl==LocalFoo
//   bind_class<T>(token: Token<T>, ctor?: Ctor<any>): void {
//     if (typeof token !== "function" && !ctor) throw new Error("bind_class: token must be a constructor (symbol token requires explicit ctor)");
//     const t: any = token as any;
//     const chosen = ctor ?? t?.[IMPL] ?? t;
//     this.providers.set(token, { kind: "class", ctor: chosen });
//   }

//   // 표준화: export surface token은 Foo로 유지하되, 실제 impl은 LocalFoo로 숨김
//   bind_export<T>(token: Token<T>, impl: Ctor<any>): void {
//     this.bind_class(token, impl);
//   }

//   bind_factory<T>(key: Token<T>, make: (app: AppContainer<Ctx>) => T): void {
//     this.providers.set(key, { kind: "factory", make });
//   }

//   // Nest useExisting/useClass 느낌: token == alias, to == real provider token
//   bind_alias<T>(token: Token<T>, to: Token<T>): void {
//     this.providers.set(token, { kind: "alias", to });
//   }

//   has<T>(key: Token<T>): boolean {
//     return this.providers.has(key) || this.singletons.has(key);
//   }

//   get<T>(ctor: Ctor<T>): T {
//     return this.get_value<T>(ctor as any);
//   }

//   get_optional<T>(key: Token<T>, fallback: T | null = null): T | null {
//     if (!this.has(key)) return fallback;
//     return this.get_value<T>(key);
//   }

//   get_value<T>(key: Token<T>): T {
//     if (this.singletons.has(key)) return this.singletons.get(key) as T;
//     if (this.building.has(key)) throw new Error(`circular dep: ${typeof key === "function" ? (key as any).name : String(key)}`);
//     this.building.add(key);

//     try {
//       const prov = this.providers.get(key);
//       if (!prov) throw new Error(`missing binding: ${typeof key === "function" ? (key as any).name : String(key)}`);

//       let v: any;
//       switch (prov.kind) {
//         case "value":
//           v = prov.value;
//           break;
//         case "instance":
//           v = prov.instance;
//           break;
//         case "factory":
//           v = prov.make(this);
//           break;
//         case "alias":
//           v = this.get_value(prov.to);
//           break;
//         case "class": {
//           const ctor: any = prov.ctor;
//           const deps = (ctor[DEPS] ?? ctor.deps ?? []) as readonly Token<any>[];
//           const args: any[] = [];
//           for (const dep of deps) args.push(this.get_value(dep));
//           v = new ctor(...args);
//           break;
//         }
//       }

//       this.singletons.set(key, v);
//       if (!this.created.has(key)) {
//         this.created.add(key);
//         this.create_order.push(key);
//       }
//       return v as T;
//     } finally {
//       this.building.delete(key);
//     }
//   }

//   // mount: create_order asc
//   async mount(ctx: Ctx): Promise<void> {
//     // instantiate all class/factory providers that exist (value/instance are already there)
//     for (const [key, prov] of this.providers) {
//       // only instantiate ctor-token providers; symbols may be values only
//       if (prov.kind === "class" || prov.kind === "factory") this.get_value(key);
//     }

//     for (const key of this.create_order) {
//       const v = this.singletons.get(key);
//       if (!v) continue;
//       try {
//         const m = (v as MaybeLifecycle<Ctx>).mount;
//         const rm = typeof m === "function" ? m.call(v, ctx) : null;
//         if (rm && typeof (rm as any).then === "function") await rm;
//       } catch (err) {
//         this.on_error?.(err, "mount", key);
//       }
//     }
//   }

//   // unmount: create_order desc (reverse creation)
//   async unmount(): Promise<void> {
//     try {
//       for (let i = this.create_order.length - 1; i >= 0; i--) {
//         const key = this.create_order[i];
//         const v = this.singletons.get(key);
//         if (!v) continue;
//         try {
//           const u = (v as MaybeLifecycle<Ctx>).unmount;
//           const ru = typeof u === "function" ? u.call(v) : null;
//           if (ru && typeof (ru as any).then === "function") await ru;
//         } catch (err) {
//           this.on_error?.(err, "unmount", key);
//         }
//       }
//     } finally {
//       // allow re-mount: keep bindings, drop runtime state
//       this.singletons.clear();
//       this.created.clear();
//       this.create_order.length = 0;
//       this.building.clear();
//     }
//   }
// }
