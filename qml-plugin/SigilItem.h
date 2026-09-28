#pragma once

#include <QElapsedTimer>
#include <QList>
#include <QMutex>
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
    // Texels per canvas unit: sharpness against memory.
    Q_PROPERTY(qreal canvasScale MEMBER m_canvasScale NOTIFY figureChanged)
    // Samples per pixel side, 1 to 4: smoothness against GPU time. 0 (the default) follows
    // the figure file's `login.supersample`, and if that is 0 too, picks for the screen.
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
    // The figure file's `login.debug`: show the readout below.
    Q_PROPERTY(bool debug READ debug NOTIFY readyChanged)
    // What is drawing it: the graphics backend and the GPU, as the driver names them.
    Q_PROPERTY(QString device READ device NOTIFY statsChanged)
    // Whether that is the CPU pretending (llvmpipe and the like): slow, whatever the theme does.
    Q_PROPERTY(bool software READ software NOTIFY statsChanged)
    // Frames per second, the longest gap between frames and the GPU's time, every two seconds.
    Q_PROPERTY(QString stats READ stats NOTIFY statsChanged)

public:
    explicit SigilItem(QQuickItem *parent = nullptr);
    ~SigilItem() override;

    QUrl figure() const { return m_figure; }
    void setFigure(const QUrl &url);
    bool ready() const { return m_engine != nullptr; }
    QString error() const { return m_error; }
    bool systemInfo() const;
    qreal checkAfter() const;
    bool debug() const;
    QString device() const { return m_device; }
    bool software() const { return m_software; }
    QString stats() const { return m_stats; }

    Q_INVOKABLE void key();
    Q_INVOKABLE void backspace();
    // True if the surge started; false if one was already running.
    Q_INVOKABLE bool surge();
    Q_INVOKABLE void fail();

signals:
    void figureChanged();
    void figureAreaChanged();
    void readyChanged();
    void statsChanged();
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
    void adopt(SigilEngine *next, const QString &error, const QByteArray &path);

    QUrl m_figure;
    QUrl m_overrides;
    QRectF m_figureArea;
    qreal m_canvasScale = 2.0;
    int m_supersample = 0;
    // Held by whichever thread is using the engine: the GUI thread passing on what the person
    // does, the render thread advancing it and making textures from it.
    mutable QMutex m_lock;
    SigilEngine *m_engine = nullptr;
    // Engines a newer load replaced, kept until the item goes in case a frame was using one.
    QList<SigilEngine *> m_retired;
    // Counts loads, so only the latest one is taken up.
    int m_loads = 0;
    QString m_error;
    // Filled in from the render thread, which knows them.
    QString m_device;
    bool m_software = false;
    QString m_stats;
    // Whether the readout is wanted, as the renderer sees it.
    bool m_debug = false;
    // Bumped on every load, so the renderer knows to upload the textures again.
    int m_generation = 0;
    QElapsedTimer m_clock;
};
