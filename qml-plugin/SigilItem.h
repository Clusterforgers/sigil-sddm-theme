#pragma once

#include <QElapsedTimer>
#include <QQuickRhiItem>
#include <QRectF>
#include <QUrl>

struct SigilEngine;

// The sigil, turning and answering, drawn by the Rust engine's shader through Qt's RHI.
//
// The item owns the engine. The GUI thread tells it what the person does; once a frame, on
// the render thread while the GUI thread waits, the renderer advances it and takes the
// uniform block. So the engine is never touched from two threads at once.
class SigilItem : public QQuickRhiItem
{
    Q_OBJECT
    QML_ELEMENT
    // The figure file. Loading it draws every layer, which takes a moment.
    Q_PROPERTY(QUrl figure READ figure WRITE setFigure NOTIFY figureChanged)
    // JSON settings laid over the figure file, if it exists: how NixOS customises the theme.
    Q_PROPERTY(QUrl overrides MEMBER m_overrides NOTIFY figureChanged)
    // Texels per canvas unit, and supersamples per pixel side: sharpness against GPU time.
    Q_PROPERTY(qreal canvasScale MEMBER m_canvasScale NOTIFY figureChanged)
    Q_PROPERTY(int supersample MEMBER m_supersample NOTIFY figureChanged)
    // Where in the item the figure is fitted; empty for all of it. The background and the
    // explosion's flash cover the whole item regardless.
    Q_PROPERTY(QRectF figureArea MEMBER m_figureArea NOTIFY figureAreaChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY readyChanged)
    Q_PROPERTY(QString error READ error NOTIFY readyChanged)
    // Whether the figure file (`login.system_info`) wants the system panel.
    Q_PROPERTY(bool systemInfo READ systemInfo NOTIFY readyChanged)
    // Seconds of the explosion to show before the password is checked (`login.check_after`).
    Q_PROPERTY(qreal checkAfter READ checkAfter NOTIFY readyChanged)

public:
    explicit SigilItem(QQuickItem *parent = nullptr);
    ~SigilItem() override;

    QUrl figure() const { return m_figure; }
    void setFigure(const QUrl &url);
    bool ready() const { return m_engine != nullptr; }
    QString error() const { return m_error; }
    bool systemInfo() const;
    qreal checkAfter() const;

    Q_INVOKABLE void key();
    Q_INVOKABLE void backspace();
    // True if the surge started; false if one was already running.
    Q_INVOKABLE bool surge();
    Q_INVOKABLE void fail();

signals:
    void figureChanged();
    void figureAreaChanged();
    void readyChanged();
    // The surge exploded: the moment to hand the password over.
    void detonated();

protected:
    QQuickRhiItemRenderer *createRenderer() override;
    void componentComplete() override;
    void hoverMoveEvent(QHoverEvent *event) override;
    void hoverLeaveEvent(QHoverEvent *event) override;

private:
    friend class SigilRenderer;
    void load();

    QUrl m_figure;
    QUrl m_overrides;
    QRectF m_figureArea;
    qreal m_canvasScale = 2.0;
    int m_supersample = 2;
    SigilEngine *m_engine = nullptr;
    QString m_error;
    // Bumped on every load, so the renderer knows to upload the textures again.
    int m_generation = 0;
    QElapsedTimer m_clock;
};
