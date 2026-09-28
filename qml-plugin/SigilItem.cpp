#include "SigilItem.h"

#include "sigil.h"

#include <QFile>
#include <QLoggingCategory>
#include <QMetaObject>
#include <QPointer>
#include <QQuickWindow>
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

    // What synchronize() took from the item for this frame.
    SigilEngine *m_engine = nullptr;
    int m_generation = -1;
    int m_built = -1;
    QByteArray m_uniforms;
    QPointer<SigilItem> m_item;

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
    }
    if (!m_engine)
        return;

    QRhiResourceUpdateBatch *batch = nullptr;
    if (m_built != m_generation) {
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
        qCInfo(lcSigil) << "textures:" << sigil_layer_count(m_engine) << "layers of" << side << "square on"
                        << m_rhi->backendName();
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
}

void SigilRenderer::synchronize(QQuickRhiItem *rhiItem)
{
    auto *item = static_cast<SigilItem *>(rhiItem);
    m_item = item;
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
    const SigilBytes u = sigil_frame(m_engine, dt, float(area.x()), float(area.y()), float(area.width()),
                                     float(area.height()), yUp ? float(px.height()) : 0.0f, 1);
    m_uniforms = QByteArray(reinterpret_cast<const char *>(u.data), qsizetype(u.len));
    if (lcSigil().isDebugEnabled()) {
        static int frames = 0;
        if (frames++ % 120 == 0) {
            const float *f = reinterpret_cast<const float *>(u.data);
            // host, then fit (after centre and host and frame): scale, offset, layers.
            qCDebug(lcSigil) << "frame" << px << "host" << f[2] << f[3] << "fit" << f[8] << f[9] << f[10] << f[11]
                             << "bytes" << u.len;
        }
    }

    if (sigil_take_detonated(m_engine))
        QMetaObject::invokeMethod(item, &SigilItem::detonated, Qt::QueuedConnection);
}

void SigilRenderer::render(QRhiCommandBuffer *cb)
{
    QRhiResourceUpdateBatch *batch = m_rhi->nextResourceUpdateBatch();
    if (m_ubuf && !m_uniforms.isEmpty())
        batch->updateDynamicBuffer(m_ubuf.get(), 0, quint32(m_uniforms.size()), m_uniforms.constData());

    if (lcSigil().isDebugEnabled()) {
        static int renders = 0;
        if (renders++ % 120 == 0)
            qCDebug(lcSigil) << "render" << renders << "pipeline" << bool(m_pipeline) << "engine" << bool(m_engine);
    }
    cb->beginPass(renderTarget(), QColor(14, 13, 12), { 1.0f, 0 }, batch);
    if (m_pipeline && m_engine) {
        const QSize px = renderTarget()->pixelSize();
        cb->setGraphicsPipeline(m_pipeline.get());
        cb->setViewport(QRhiViewport(0, 0, float(px.width()), float(px.height())));
        cb->setShaderResources();
        cb->draw(3);
    }
    cb->endPass();
}

SigilItem::SigilItem(QQuickItem *parent)
    : QQuickRhiItem(parent)
{
    setAcceptHoverEvents(true);
}

SigilItem::~SigilItem()
{
    sigil_engine_free(m_engine);
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

void SigilItem::load()
{
    if (m_figure.isEmpty())
        return;
    const QString path = m_figure.isLocalFile() ? m_figure.toLocalFile() : m_figure.toString();
    char *err = nullptr;
    SigilEngine *next = sigil_engine_new(QFile::encodeName(path).constData(), float(m_canvasScale),
                                         uint32_t(m_supersample), &err);
    if (!next) {
        m_error = QString::fromUtf8(err);
        sigil_string_free(err);
        qWarning("sigil: cannot load %s: %s", qPrintable(path), qPrintable(m_error));
        emit readyChanged();
        return;
    }
    sigil_engine_free(m_engine);
    m_engine = next;
    m_error.clear();
    ++m_generation;
    m_clock.invalidate();
    emit readyChanged();
    update();
}

void SigilItem::key()
{
    if (m_engine)
        sigil_key(m_engine);
}

void SigilItem::backspace()
{
    if (m_engine)
        sigil_backspace(m_engine);
}

bool SigilItem::surge()
{
    return m_engine && sigil_surge(m_engine);
}

void SigilItem::fail()
{
    if (m_engine)
        sigil_fail(m_engine);
}

void SigilItem::hoverMoveEvent(QHoverEvent *event)
{
    if (!m_engine)
        return;
    // The engine thinks in the frame's pixels, which are the item's scaled by the screen.
    const qreal dpr = window() ? window()->effectiveDevicePixelRatio() : 1.0;
    const QPointF p = event->position() * dpr;
    sigil_pointer(m_engine, 1, float(p.x()), float(p.y()));
}

void SigilItem::hoverLeaveEvent(QHoverEvent *)
{
    if (m_engine)
        sigil_pointer(m_engine, 0, 0, 0);
}
