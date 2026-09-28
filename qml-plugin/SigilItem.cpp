#include "SigilItem.h"

#include "sigil.h"

#include <QCoreApplication>
#include <QFile>
#include <QLoggingCategory>
#include <QMetaObject>
#include <QPointer>
#include <QQuickWindow>
#include <QThread>
#include <rhi/qrhi.h>

#include <memory>

Q_LOGGING_CATEGORY(lcSigil, "sigil", QtWarningMsg)

namespace {

QShader loadShader(const QString &name)
{
    QFile f(name);
    if (!f.open(QIODevice::ReadOnly))
        qFatal("sigil: shader %s is missing from the plugin", qPrintable(name));
    return QShader::fromSerialized(f.readAll());
}

// Bytes per texel of each texture the engine hands over.
int texelBytes(QRhiTexture::Format format)
{
    switch (format) {
    case QRhiTexture::RGBA8: return 4;
    case QRhiTexture::RG8: return 2;
    default: return 1;
    }
}

} // namespace

// The GPU side: the three texture arrays, the uniform buffer, and one full-screen pass.
// A port of src/gpu/pipeline.rs; the shader is the same one, translated.
class SigilRenderer : public QQuickRhiItemRenderer
{
protected:
    void initialize(QRhiCommandBuffer *cb) override;
    void synchronize(QQuickRhiItem *item) override;
    void render(QRhiCommandBuffer *cb) override;

private:
    // One array texture from each layer's mip chain, as `fetch` returns it.
    QRhiTexture *layerArray(QRhiResourceUpdateBatch *batch, QRhiTexture::Format format, QRhiTexture::Flags flags,
                            int side, SigilBytes (*fetch)(SigilEngine *, uint32_t));
    void release();
    // Every two seconds, with `sigil.info` logging on: frames per second, the longest gap
    // between frames (what "laggy" means to someone watching), the engine's CPU time and,
    // under QSG_RHI_PROFILE=1, the GPU's time for the pass.
    void report(QSize px, qint64 gapNs, qint64 cpuNs);
    QElapsedTimer m_since;
    int m_frames = 0;
    int m_ss = 1;
    // Whether the device drawing this has been named yet.
    void nameDevice();
    bool m_named = false;
    qint64 m_worstGap = 0, m_worstCpu = 0;
    double m_gpu = 0;

    // What synchronize() took from the item for this frame.
    SigilEngine *m_engine = nullptr;
    int m_generation = -1;
    int m_built = -1;
    QByteArray m_uniforms;
    QPointer<SigilItem> m_item;
    // The item's lock on the engine, for the work done outside synchronize().
    QMutex *m_lock = nullptr;

    QRhi *m_rhi = nullptr;
    std::unique_ptr<QRhiBuffer> m_ubuf;
    std::unique_ptr<QRhiTexture> m_art, m_glow, m_reveal;
    std::unique_ptr<QRhiSampler> m_smooth, m_exact;
    std::unique_ptr<QRhiShaderResourceBindings> m_srb;
    std::unique_ptr<QRhiGraphicsPipeline> m_pipeline;
    QRhiRenderPassDescriptor *m_passFor = nullptr;
};

void SigilRenderer::release()
{
    m_pipeline.reset();
    m_srb.reset();
    m_art.reset();
    m_glow.reset();
    m_reveal.reset();
    m_ubuf.reset();
    m_passFor = nullptr;
}

QRhiTexture *SigilRenderer::layerArray(QRhiResourceUpdateBatch *batch, QRhiTexture::Format format,
                                       QRhiTexture::Flags flags, int side, SigilBytes (*fetch)(SigilEngine *, uint32_t))
{
    const int layers = int(sigil_layer_count(m_engine));
    QRhiTexture *tex = m_rhi->newTextureArray(format, layers, QSize(side, side), 1, flags);
    tex->create();
    QList<QRhiTextureUploadEntry> entries;
    for (int layer = 0; layer < layers; ++layer) {
        const SigilBytes chain = fetch(m_engine, uint32_t(layer));
        // The chain is every level back to back, level 0 first, each tightly packed.
        size_t offset = 0;
        for (uint32_t level = 0; level < chain.levels; ++level) {
            const int s = qMax(1, side >> level);
            const size_t bytes = size_t(s) * size_t(s) * size_t(texelBytes(format));
            if (offset + bytes > chain.len)
                break;
            QRhiTextureSubresourceUploadDescription d(reinterpret_cast<const char *>(chain.data + offset), quint32(bytes));
            d.setSourceSize(QSize(s, s));
            // Rows are packed with no padding, whatever the width.
            d.setDataStride(quint32(s * texelBytes(format)));
            entries.append(QRhiTextureUploadEntry(layer, int(level), d));
            offset += bytes;
        }
    }
    QRhiTextureUploadDescription desc;
    desc.setEntries(entries.cbegin(), entries.cend());
    batch->uploadTexture(tex, desc);
    return tex;
}

void SigilRenderer::initialize(QRhiCommandBuffer *cb)
{
    if (m_rhi != rhi()) {
        release();
        m_smooth.reset();
        m_exact.reset();
        m_built = -1;
        m_rhi = rhi();
        m_named = false;
    }
    if (!m_engine)
        return;

    QRhiResourceUpdateBatch *batch = nullptr;
    if (m_built != m_generation) {
        QMutexLocker lock(m_lock);
        QElapsedTimer took;
        took.start();
        release();
        batch = m_rhi->nextResourceUpdateBatch();
        const int side = int(sigil_layer_side(m_engine));
        // The artwork decodes from sRGB as it is sampled, as in the wgpu viewer; the glow is
        // already linear; the reveal map is packed bytes, read exactly.
        m_art.reset(layerArray(batch, QRhiTexture::RGBA8, QRhiTexture::MipMapped | QRhiTexture::sRGB, side,
                               sigil_layer_art));
        m_glow.reset(layerArray(batch, QRhiTexture::R8, QRhiTexture::MipMapped, side, sigil_layer_glow));
        m_reveal.reset(layerArray(batch, QRhiTexture::RG8, {}, int(sigil_reveal_side(m_engine)), sigil_layer_reveal));

        m_ubuf.reset(m_rhi->newBuffer(QRhiBuffer::Dynamic, QRhiBuffer::UniformBuffer, quint32(sigil_uniforms_size())));
        m_ubuf->create();

        if (!m_smooth) {
            // Trilinear, so the bloom slides smoothly between pyramid levels.
            m_smooth.reset(m_rhi->newSampler(QRhiSampler::Linear, QRhiSampler::Linear, QRhiSampler::Linear,
                                             QRhiSampler::ClampToEdge, QRhiSampler::ClampToEdge));
            m_smooth->create();
            m_exact.reset(m_rhi->newSampler(QRhiSampler::Nearest, QRhiSampler::Nearest, QRhiSampler::None,
                                            QRhiSampler::ClampToEdge, QRhiSampler::ClampToEdge));
            m_exact->create();
        }

        const auto frag = QRhiShaderResourceBinding::FragmentStage;
        m_srb.reset(m_rhi->newShaderResourceBindings());
        m_srb->setBindings({
            QRhiShaderResourceBinding::uniformBuffer(0, frag, m_ubuf.get()),
            QRhiShaderResourceBinding::sampledTexture(1, frag, m_art.get(), m_smooth.get()),
            QRhiShaderResourceBinding::sampledTexture(3, frag, m_glow.get(), m_smooth.get()),
            QRhiShaderResourceBinding::sampledTexture(4, frag, m_reveal.get(), m_exact.get()),
        });
        m_srb->create();
        m_built = m_generation;
        // The upload copied them; the CPU's copies are a hundred-odd megabytes to let go of.
        sigil_release_pyramids(m_engine);
        qCInfo(lcSigil) << "textures:" << sigil_layer_count(m_engine) << "layers of" << side << "square on"
                        << m_rhi->backendName() << "prepared in" << took.elapsed() << "ms";
    }

    if (!m_pipeline || m_passFor != renderTarget()->renderPassDescriptor()) {
        m_pipeline.reset(m_rhi->newGraphicsPipeline());
        m_pipeline->setShaderStages({
            { QRhiShaderStage::Vertex, loadShader(QStringLiteral(":/qt/qml/Sigil/shaders/sigil.vert.qsb")) },
            { QRhiShaderStage::Fragment, loadShader(QStringLiteral(":/qt/qml/Sigil/shaders/sigil.frag.qsb")) },
        });
        // The triangle is made up in the vertex shader from its index; there are no inputs.
        m_pipeline->setVertexInputLayout({});
        m_pipeline->setShaderResourceBindings(m_srb.get());
        m_pipeline->setRenderPassDescriptor(renderTarget()->renderPassDescriptor());
        m_pipeline->setSampleCount(renderTarget()->sampleCount());
        const bool ok = m_pipeline->create();
        qCInfo(lcSigil) << "pipeline" << (ok ? "created" : "FAILED") << "for" << renderTarget()->pixelSize();
        m_passFor = renderTarget()->renderPassDescriptor();
    }

    if (batch)
        cb->resourceUpdate(batch);

    if (!m_named)
        nameDevice();
}

// Say once what is drawing the figure. A login screen that has fallen back to rendering on
// the CPU is slow however little the theme asks of it, so that is said even without debug.
void SigilRenderer::nameDevice()
{
    if (!m_item)
        return;
    m_named = true;
    const QRhiDriverInfo d = m_rhi->driverInfo();
    const QString name = QString::fromUtf8(d.deviceName);
    const bool software = d.deviceType == QRhiDriverInfo::CpuDevice
        || name.contains(QLatin1String("llvmpipe"), Qt::CaseInsensitive)
        || name.contains(QLatin1String("softpipe"), Qt::CaseInsensitive)
        || name.contains(QLatin1String("lavapipe"), Qt::CaseInsensitive)
        || name.contains(QLatin1String("swrast"), Qt::CaseInsensitive);
    const QString device = QStringLiteral("%1 on %2").arg(QString::fromLatin1(m_rhi->backendName()), name);
    if (software)
        qCWarning(lcSigil) << "drawing on the CPU, not a GPU:" << device;
    else
        qCInfo(lcSigil) << "drawing with" << device;
    QMetaObject::invokeMethod(m_item.get(), [item = m_item, device, software] {
        if (!item)
            return;
        item->m_device = device;
        item->m_software = software;
        emit item->statsChanged();
    }, Qt::QueuedConnection);
}

void SigilRenderer::synchronize(QQuickRhiItem *rhiItem)
{
    auto *item = static_cast<SigilItem *>(rhiItem);
    m_item = item;
    m_lock = &item->m_lock;
    QMutexLocker lock(m_lock);
    m_engine = item->m_engine;
    m_generation = item->m_generation;
    if (!m_engine)
        return;

    // It never stops moving. Ask for the next frame from the GUI thread: only an update
    // there brings another synchronize(), which is where the engine may be advanced.
    QMetaObject::invokeMethod(item, [item] { item->update(); }, Qt::QueuedConnection);

    // Time since the last frame; a long gap (the screen was off) is not a leap forward.
    const qint64 ns = item->m_clock.isValid() ? item->m_clock.nsecsElapsed() : 0;
    item->m_clock.restart();
    const float dt = qMin(float(ns) * 1e-9f, 0.1f);

    const QSize px = renderTarget() ? renderTarget()->pixelSize() : QSize();
    if (px.isEmpty())
        return;
    const bool yUp = rhi() && rhi()->isYUpInFramebuffer();
    // The item's texture is plain RGBA8, stored as given: encode sRGB in the shader.
    // The figure's area, from the item's units into the frame's pixels.
    const qreal k = item->width() > 0 ? px.width() / item->width() : 1.0;
    const QRectF area = item->m_figureArea.isEmpty() ? QRectF(QPointF(), px)
                                                      : QRectF(item->m_figureArea.topLeft() * k, item->m_figureArea.size() * k);
    // Samples per pixel side: what the item says, else the figure file, else one sample on a
    // screen of 3.5 megapixels or more, whose pixels are too small to need more, and two
    // below. Each step up costs the GPU about as much again as the whole figure at one.
    int ss = item->m_supersample > 0 ? item->m_supersample : int(sigil_login_supersample(m_engine));
    if (ss <= 0)
        ss = qint64(px.width()) * px.height() >= 3500000 ? 1 : 2;
    sigil_set_supersample(m_engine, uint32_t(ss));
    m_ss = ss;
    QElapsedTimer cpu;
    cpu.start();
    const SigilBytes u = sigil_frame(m_engine, dt, float(area.x()), float(area.y()), float(area.width()),
                                     float(area.height()), yUp ? float(px.height()) : 0.0f, 1);
    m_uniforms = QByteArray(reinterpret_cast<const char *>(u.data), qsizetype(u.len));
    if (item->m_debug || lcSigil().isInfoEnabled())
        report(px, ns, cpu.nsecsElapsed());

    if (sigil_take_detonated(m_engine))
        QMetaObject::invokeMethod(item, &SigilItem::detonated, Qt::QueuedConnection);
}

void SigilRenderer::report(QSize px, qint64 gapNs, qint64 cpuNs)
{
    if (!m_since.isValid())
        m_since.start();
    ++m_frames;
    m_worstGap = qMax(m_worstGap, gapNs);
    m_worstCpu = qMax(m_worstCpu, cpuNs);
    if (m_since.elapsed() < 2000)
        return;
    // GPU time needs timestamps, which Qt only asks the driver for under QSG_RHI_PROFILE=1.
    const QString gpu = m_gpu > 0 ? QString::asprintf("%.2f ms", m_gpu * 1000.0) : QStringLiteral("n/a (QSG_RHI_PROFILE=1)");
    const QString stats = QString::asprintf("%dx%d, %d sample%s per pixel side: %.0f fps, worst gap %.1f ms, engine %.2f ms, gpu ",
                                            px.width(), px.height(), m_ss, m_ss == 1 ? "" : "s",
                                            m_frames * 1000.0 / m_since.elapsed(), m_worstGap * 1e-6, m_worstCpu * 1e-6)
        + gpu;
    qCInfo(lcSigil).noquote() << stats;
    if (m_item) {
        QMetaObject::invokeMethod(m_item.get(), [item = m_item, stats] {
            if (!item)
                return;
            item->m_stats = stats;
            emit item->statsChanged();
        }, Qt::QueuedConnection);
    }
    m_since.restart();
    m_frames = 0;
    m_worstGap = m_worstCpu = 0;
    m_gpu = 0;
}

void SigilRenderer::render(QRhiCommandBuffer *cb)
{
    QRhiResourceUpdateBatch *batch = m_rhi->nextResourceUpdateBatch();
    if (m_ubuf && !m_uniforms.isEmpty())
        batch->updateDynamicBuffer(m_ubuf.get(), 0, quint32(m_uniforms.size()), m_uniforms.constData());

    cb->beginPass(renderTarget(), QColor(14, 13, 12), { 1.0f, 0 }, batch);
    if (m_pipeline && m_engine) {
        const QSize px = renderTarget()->pixelSize();
        cb->setGraphicsPipeline(m_pipeline.get());
        cb->setViewport(QRhiViewport(0, 0, float(px.width()), float(px.height())));
        cb->setShaderResources();
        cb->draw(3);
    }
    cb->endPass();
    // With QSG_RHI_PROFILE=1 the GPU's own time for a frame comes back a frame or two later.
    m_gpu = qMax(m_gpu, cb->lastCompletedGpuTime());
}

SigilItem::SigilItem(QQuickItem *parent)
    : QQuickRhiItem(parent)
{
    setAcceptHoverEvents(true);
}

SigilItem::~SigilItem()
{
    sigil_engine_free(m_engine);
    for (SigilEngine *e : std::as_const(m_retired))
        sigil_engine_free(e);
}

QQuickRhiItemRenderer *SigilItem::createRenderer()
{
    return new SigilRenderer;
}

void SigilItem::setFigure(const QUrl &url)
{
    if (url == m_figure)
        return;
    m_figure = url;
    emit figureChanged();
    if (isComponentComplete())
        load();
}

void SigilItem::componentComplete()
{
    QQuickRhiItem::componentComplete();
    load();
}

// Loading draws every layer and builds its texture pyramids, most of a second of work. It
// happens on a thread of its own so the login screen never stops for it; until it is done
// the item draws only the background, and `ready` says when the figure is there.
void SigilItem::load()
{
    if (m_figure.isEmpty())
        return;
    const auto local = [](const QUrl &u) { return QFile::encodeName(u.isLocalFile() ? u.toLocalFile() : u.toString()); };
    const QByteArray path = local(m_figure);
    const QByteArray overrides = m_overrides.isEmpty() ? QByteArray() : local(m_overrides);
    const float scale = float(m_canvasScale);
    const int load = ++m_loads;
    const QPointer<SigilItem> self(this);

    QThread *worker = QThread::create([=] {
        QElapsedTimer took;
        took.start();
        char *err = nullptr;
        SigilEngine *next =
            sigil_engine_new(path.constData(), overrides.isEmpty() ? nullptr : overrides.constData(), scale, 1, &err);
        const QString error = err ? QString::fromUtf8(err) : QString();
        sigil_string_free(err);
        qCInfo(lcSigil) << "figure loaded in" << took.elapsed() << "ms";
        // Back on the GUI thread. A newer load, or the item being gone, makes this one moot.
        QMetaObject::invokeMethod(qApp, [self, next, error, load, path] {
            if (!self || load != self->m_loads) {
                sigil_engine_free(next);
                return;
            }
            self->adopt(next, error, path);
        }, Qt::QueuedConnection);
    });
    connect(worker, &QThread::finished, worker, &QObject::deleteLater);
    worker->start();
}

void SigilItem::adopt(SigilEngine *next, const QString &error, const QByteArray &path)
{
    if (!next) {
        m_error = error;
        qWarning("sigil: cannot load %s: %s", path.constData(), qPrintable(m_error));
        emit readyChanged();
        return;
    }
    {
        QMutexLocker lock(&m_lock);
        // The renderer may still be reading the old one this frame; it goes with the item.
        if (m_engine)
            m_retired.append(m_engine);
        m_engine = next;
    }
    // The readout, and the journal report with it, which otherwise needs a logging rule.
    m_debug = sigil_login_debug(next);
    if (m_debug)
        const_cast<QLoggingCategory &>(lcSigil()).setEnabled(QtInfoMsg, true);
    m_error.clear();
    ++m_generation;
    m_clock.invalidate();
    emit readyChanged();
    update();
}

bool SigilItem::systemInfo() const
{
    QMutexLocker lock(&m_lock);
    return m_engine && sigil_system_info(m_engine);
}

bool SigilItem::debug() const
{
    return m_debug;
}

qreal SigilItem::checkAfter() const
{
    QMutexLocker lock(&m_lock);
    return m_engine ? sigil_check_after(m_engine) : 0.5;
}

void SigilItem::key()
{
    QMutexLocker lock(&m_lock);
    if (m_engine)
        sigil_key(m_engine);
}

void SigilItem::backspace()
{
    QMutexLocker lock(&m_lock);
    if (m_engine)
        sigil_backspace(m_engine);
}

bool SigilItem::surge()
{
    QMutexLocker lock(&m_lock);
    return m_engine && sigil_surge(m_engine);
}

void SigilItem::fail()
{
    QMutexLocker lock(&m_lock);
    if (m_engine)
        sigil_fail(m_engine);
}

void SigilItem::hoverMoveEvent(QHoverEvent *event)
{
    // The engine thinks in the frame's pixels, which are the item's scaled by the screen.
    const qreal dpr = window() ? window()->effectiveDevicePixelRatio() : 1.0;
    const QPointF p = event->position() * dpr;
    QMutexLocker lock(&m_lock);
    if (m_engine)
        sigil_pointer(m_engine, 1, float(p.x()), float(p.y()));
}

void SigilItem::hoverLeaveEvent(QHoverEvent *)
{
    QMutexLocker lock(&m_lock);
    if (m_engine)
        sigil_pointer(m_engine, 0, 0, 0);
}
