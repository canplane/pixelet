// cpp/xpute/link/link.cpp

#include "link.hpp"

#include <string>

namespace xpute::link {

namespace {

constexpr std::int32_t status(Errno e) noexcept {
    return static_cast<std::int32_t>(e);
}

// Whether `a` and `b` are one token.
bool same(const Token &a, const Token &b) noexcept {
    if (a.variant.index() != b.variant.index()) return false;
    if (a.variant.index() == 0) return std::get<0>(a.variant).val == std::get<0>(b.variant).val;
    return std::get<1>(a.variant).val == std::get<1>(b.variant).val;
}

// Whether `uri` starts with `prefix`.
bool starts_with(std::span<const Token> uri, std::span<const Token> prefix) noexcept {
    if (prefix.size() > uri.size()) return false;
    for (std::size_t k = 0; k < prefix.size(); k++)
        if (!same(uri[k], prefix[k])) return false;
    return true;
}

// A copy of a packet, which its answer owns.
std::optional<std::vector<std::uint8_t>> copy(std::optional<std::span<const std::uint8_t>> pkt) {
    if (!pkt) return std::nullopt;
    return std::vector<std::uint8_t>(pkt->begin(), pkt->end());
}

} // namespace

void Side::mount(Handle h, std::vector<Token> prefix, Opener open) {
    mounts_.emplace_back(h, std::move(prefix), std::move(open));
}

void Side::mount_named(Handle h, std::unique_ptr<Object> o) {
    bool fresh = named_.emplace(h, std::move(o)).second;
    ensure(fresh, Errno::eexist, h);
}

Result<Handle> Side::open(std::span<const Token> uri) {
    Args args = Packet{8, [uri](Lower &w) { store_uri(w, 0, uri); }};
    auto [result, data] = call(ROOT, OPEN, args);
    return answer(result, data, [](Lift &r) { return r.get<std::uint32_t>(0); });
}

void Side::drop(Handle h) {
    send(h, DROP, std::nullopt, nullptr);
}

std::pair<std::int32_t, std::optional<std::span<const std::uint8_t>>> Side::call(Handle h, Method m, const Args &args) {
    auto reply = out(tag(), h, m, false, 0, args);
    ensure(reply.first != status(Errno::einprogress), Errno::einprogress, h, m);
    return reply;
}

Tag Side::send(Handle h, Method m, const Args &args, Answer answer) {
    Tag t = tag();
    if (answer) pending_.emplace(t, std::move(answer));
    auto [result, data] = out(t, h, m, false, 0, args);
    if (result != status(Errno::einprogress)) {
        if (auto it = pending_.find(t); it != pending_.end()) {
            Answer a = std::move(it->second);
            pending_.erase(it);
            due_.push_back(Due{std::move(a), result, copy(data)});
        }
    }
    return t;
}

void Side::cancel(Tag t) {
    if (pending_.contains(t)) send(ROOT, CANCEL, Packet{4, [t](Lower &w) { w.put<std::uint32_t>(0, t); }}, nullptr);
}

void Side::on_cancel(Tag t, std::function<void()> stop) {
    cancels_.insert_or_assign(t, std::move(stop));
}

void Side::task_return(Tag t, std::int32_t result, const Data &data) {
    cancels_.erase(t);
    if (!crossing_) {
        out(t, ROOT, 0, true, result, data);
        return;
    }
    if (!data) {
        completions_.push_back(Completion{t, Written{result, std::nullopt}});
        return;
    }
    std::vector<std::uint8_t> buf(layout().cap - HDR_SZ);
    Lower w(buf, data->fixed_sz);
    data->write(w);
    Result<std::size_t> n = w.finish();
    if (!n) {
        completions_.push_back(Completion{t, n.error()});
        return;
    }
    buf.resize(*n);
    completions_.push_back(Completion{t, Written{result, std::move(buf)}});
}

void Side::task_err(Tag t, Error e) {
    cancels_.erase(t);
    if (crossing_) {
        completions_.push_back(Completion{t, std::move(e)});
        return;
    }
    out_err(t, e);
}

namespace {

// Writes the fixed part of the error `e`'s packet from 0 on, and its file and
// message past it.
void lower_err(Lower &w, const Error &e) noexcept {
    ErrorPacket::store_of(w, 0, e.file, e.line, e.msg.empty() ? std::nullopt : std::optional<std::string_view>(e.msg));
}

} // namespace

void Side::out_err(Tag t, const Error &e) {
    out(t, ROOT, 0, true, status(e.errnum), Packet{ErrorPacket::SZ, [&e](Lower &w) { lower_err(w, e); }});
}

std::int32_t Side::recv(std::uint64_t at) {
    std::uint32_t cap = layout().cap;
    Frame f = Frame::at(at);
    Result<std::optional<std::span<const std::uint8_t>>> pkt = f.pkt(at, cap);
    if (f.res) {
        if (auto it = pending_.find(f.tag); it != pending_.end()) {
            Answer a = std::move(it->second);
            pending_.erase(it);
            due_.push_back(Due{std::move(a), f.result, pkt ? copy(*pkt) : std::nullopt});
        }
        return 0;
    }
    // The args and the answer lie in the frame's packet, the answer written
    // once the args are read.
    Lower w(mem::bytes(at + HDR_SZ, cap - HDR_SZ));
    Frame words = f;
    words.res = true;
    words.result = pkt ? run(f.handle, f.method, f.tag, Lift((*pkt).value_or(std::span<const std::uint8_t>{})), w)
                       : store_err(w, pkt.error());
    // An answer past the cap is refused as a later one is (`out`), its error
    // laid in its place.
    Result<std::size_t> n = w.finish();
    if (!n) {
        words.result = store_err(w, n.error());
        n = w.finish();
    }
    words.pkt_len = static_cast<std::uint32_t>(or_bug(n, Errno::emsgsize));
    words.lay(at);
    return words.result;
}

void Side::run_due() {
    while (!completions_.empty()) {
        Completion c = std::move(completions_.front());
        completions_.pop_front();
        if (!c.answer) {
            out_err(c.tag, c.answer.error());
            continue;
        }
        Args pkt;
        if (c.answer->pkt) {
            const std::vector<std::uint8_t> &bytes = *c.answer->pkt;
            pkt = Packet{bytes.size(), [&bytes](Lower &w) {
                             for (std::size_t k = 0; k < bytes.size(); k++) w.put<std::uint8_t>(k, bytes[k]);
                         }};
        }
        out(c.tag, ROOT, 0, true, c.answer->result, pkt);
    }
    while (!due_.empty()) {
        Due d = std::move(due_.front());
        due_.pop_front();
        d.answer(d.result, std::move(d.data));
    }
}

std::int32_t Side::run(Handle h, Method m, Tag tag, Lift args, Lower &ret) {
    if (h == ROOT && m == OPEN) {
        Result<Handle> o = opened(args);
        if (!o) return store_err(ret, o.error());
        ret.start(4);
        ret.put<std::uint32_t>(0, *o);
        return 0;
    }
    if (h == ROOT && m == CANCEL) {
        Tag t = args.get<std::uint32_t>(0);
        if (Result<void> read = args.finish(); !read) return store_err(ret, read.error());
        if (auto it = cancels_.find(t); it != cancels_.end()) {
            std::function<void()> stop = std::move(it->second);
            cancels_.erase(it);
            stop();
        }
        return 0;
    }
    if (m == DROP) return objects_.remove(h) ? 0 : store_err(ret, make_error(Errno::ebadf));
    // A named handle no interface is mounted under is no object there; an
    // issued one no longer held is a bad handle. Nothing drops an object
    // during its call: the host's drop comes only in an entry of its own.
    if ((h & NAMED) != 0) {
        auto it = named_.find(h);
        if (it == named_.end()) return store_err(ret, make_error(Errno::enoent));
        return it->second->call(m, tag, args, ret);
    }
    std::unique_ptr<Object> *o = objects_.get(h);
    if (o == nullptr) return store_err(ret, make_error(Errno::ebadf));
    return (*o)->call(m, tag, args, ret);
}

Result<Handle> Side::opened(Lift &args) {
    std::vector<Token> uri = load_uri(args, 0);
    if (Result<void> read = args.finish(); !read) return read.error();
    // The longest matching mount wins, the first mounted of those as long.
    const Opener *open = nullptr;
    std::size_t longest = 0;
    for (const auto &[i, prefix, o] : mounts_) {
        if (starts_with(uri, prefix) && (open == nullptr || prefix.size() > longest)) {
            open = &o;
            longest = prefix.size();
        }
    }
    if (open == nullptr) return make_error(Errno::enoent);
    // A copy, as an opener may mount another.
    Opener opener = *open;
    Handle h = objects_.issue();
    Result<std::unique_ptr<Object>> o = opener(uri, h);
    if (!o) {
        objects_.remove(h);
        return o.error();
    }
    objects_.put(h, std::move(*o));
    return h;
}

Tag Side::tag() noexcept {
    Tag t = next_;
    while (pending_.contains(t)) t++;
    next_ = t + 1;
    return t;
}

Result<void> Side::lay(std::uint64_t at, Frame words, const Args &pkt) {
    std::uint32_t cap = layout().cap;
    words.pkt_len = 0;
    if (pkt) {
        Lower w(mem::bytes(at + HDR_SZ, cap - HDR_SZ), pkt->fixed_sz);
        pkt->write(w);
        Result<std::size_t> n = w.finish();
        if (!n) return n.error();
        words.pkt_len = static_cast<std::uint32_t>(*n);
    }
    words.lay(at);
    return {};
}

// Lays a frame in this side's box and crosses with it, and answers what the
// callee laid there: its result, and its return data. A later answer past
// the cap is answered EMSGSIZE, its error laid in its place, so its request
// still hears; args this side writes past it are a bug.
std::pair<std::int32_t, std::optional<std::span<const std::uint8_t>>> Side::out(Tag tag, Handle h, Method m, bool res, std::int32_t result, const Args &pkt) {
    Layout l = layout();
    std::uint64_t at = out_box_(l);
    Frame words{.tag = tag, .res = res, .handle = h, .method = m, .result = result, .pkt_len = 0};
    if (Result<void> laid = lay(at, words, pkt); !laid) {
        ensure(res, Errno::emsgsize, h, m);
        const Error &e = laid.error();
        words.result = status(e.errnum);
        ensure(lay(at, words, Packet{ErrorPacket::SZ, [&e](Lower &w) { lower_err(w, e); }}).ok(), Errno::emsgsize);
    }
    std::int32_t r = crossed([&] { return cross_(at); });
    Frame f = Frame::at(at);
    std::optional<std::span<const std::uint8_t>> data;
    if (f.res && !res) {
        if (auto p = f.pkt(at, l.cap); p) data = *p;
    }
    return {r, data};
}

std::vector<Token> uri(std::initializer_list<std::string_view> names) {
    std::vector<Token> out;
    for (std::string_view n : names) out.push_back(Token{Token::Str{std::string(n)}});
    return out;
}

void store_uri(Lower &w, std::size_t at, std::span<const Token> uri) {
    if (auto base = w.list(at, uri.size(), Token::SZ, Token::ALIGN))
        for (std::size_t k = 0; k < uri.size(); k++) uri[k].store(w, *base + k * Token::SZ);
}

std::vector<Token> load_uri(Lift &r, std::size_t at) {
    auto [base, len] = r.list(at, Token::SZ, Token::ALIGN);
    std::vector<Token> uri;
    uri.reserve(len);
    for (std::size_t k = 0; k < len; k++) uri.push_back(Token::load(r, base + k * Token::SZ));
    return uri;
}

std::int32_t store_err(Lower &w, const Error &e) noexcept {
    w.start(ErrorPacket::SZ);
    lower_err(w, e);
    return status(e.errnum);
}

Result<void> answer(std::int32_t result, std::optional<std::span<const std::uint8_t>> data) noexcept {
    if (result == 0) return {};
    Errno errnum = or_bug(of_Errno(result), Errno::eproto);
    if (!data) return make_error(errnum);
    Lift r{*data};
    ErrorPacket pkt = ErrorPacket::load(r, 0);
    if (Result<void> read = r.finish(); !read) return read.error();
    return make_error(errnum, pkt.file, pkt.line, pkt.msg.value_or(std::string()));
}

} // namespace xpute::link
