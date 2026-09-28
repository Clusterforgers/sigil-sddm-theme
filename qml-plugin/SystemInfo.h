#pragma once

#include <QObject>
#include <QTimer>
#include <QtQml/qqmlregistration.h>

// What the machine is and how it is doing, for the login screen's side panel. Read from
// /proc and /sys every couple of seconds; anything the machine does not have (a battery,
// a temperature sensor) reads as unknown, and the panel leaves its line out.
class SystemInfo : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(QString os READ os CONSTANT)
    Q_PROPERTY(QString host READ host CONSTANT)
    Q_PROPERTY(QString kernel READ kernel CONSTANT)
    Q_PROPERTY(QString cpu READ cpu CONSTANT)
    Q_PROPERTY(int cores READ cores CONSTANT)
    // 0 to 1, over the last interval.
    Q_PROPERTY(qreal cpuLoad READ cpuLoad NOTIFY changed)
    // Degrees Celsius, or -1.
    Q_PROPERTY(qreal temperature READ temperature NOTIFY changed)
    // Bytes.
    Q_PROPERTY(qreal memoryUsed READ memoryUsed NOTIFY changed)
    Q_PROPERTY(qreal memoryTotal READ memoryTotal NOTIFY changed)
    // Seconds since boot.
    Q_PROPERTY(qreal uptime READ uptime NOTIFY changed)
    // Percent, or -1 without a battery; and whether it is charging.
    Q_PROPERTY(int battery READ battery NOTIFY changed)
    Q_PROPERTY(bool charging READ charging NOTIFY changed)

public:
    explicit SystemInfo(QObject *parent = nullptr);

    QString os() const { return m_os; }
    QString host() const { return m_host; }
    QString kernel() const { return m_kernel; }
    QString cpu() const { return m_cpu; }
    int cores() const { return m_cores; }
    qreal cpuLoad() const { return m_cpuLoad; }
    qreal temperature() const { return m_temperature; }
    qreal memoryUsed() const { return m_memoryUsed; }
    qreal memoryTotal() const { return m_memoryTotal; }
    qreal uptime() const { return m_uptime; }
    int battery() const { return m_battery; }
    bool charging() const { return m_charging; }

signals:
    void changed();

private:
    void refresh();

    QString m_os, m_host, m_kernel, m_cpu;
    int m_cores = 0;
    qreal m_cpuLoad = 0, m_temperature = -1, m_memoryUsed = 0, m_memoryTotal = 0, m_uptime = 0;
    int m_battery = -1;
    bool m_charging = false;
    // The CPU counters at the last reading, for the load over the interval since.
    quint64 m_busy = 0, m_total = 0;
    QString m_thermalZone;
    QString m_batteryDir;
    QTimer m_timer;
};
