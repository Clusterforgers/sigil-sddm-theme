#include "SystemInfo.h"

#include <QDir>
#include <QFile>
#include <QRegularExpression>
#include <QSysInfo>
#include <QThread>

namespace {

QString readText(const QString &path)
{
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly))
        return {};
    return QString::fromUtf8(f.readAll()).trimmed();
}

// "AMD Ryzen 7 7840U w/ Radeon 780M Graphics" -> "AMD Ryzen 7 7840U": the part a person
// would say out loud.
QString shortCpuName(QString name)
{
    static const QRegularExpression noise(
        QStringLiteral(R"(\((R|TM|tm|r)\)|\bCPU\b|\bProcessor\b|\s+(w/|with)\s.*$|\s*@.*$|\d+-Core\b)"));
    name.replace(noise, QString());
    return name.simplified();
}

// The temperature sensor that best stands for the CPU: the package sensor on Intel, the
// ACPI zone otherwise, else whichever comes first.
QString findThermalZone()
{
    const QDir dir(QStringLiteral("/sys/class/thermal"));
    QString fallback;
    for (const QString &zone : dir.entryList({ QStringLiteral("thermal_zone*") }, QDir::Dirs)) {
        const QString path = dir.filePath(zone);
        const QString type = readText(path + QStringLiteral("/type"));
        if (type == QLatin1String("x86_pkg_temp") || type.contains(QLatin1String("cpu"), Qt::CaseInsensitive))
            return path;
        if (fallback.isEmpty() || type == QLatin1String("acpitz"))
            fallback = path;
    }
    return fallback;
}

QString findBattery()
{
    const QDir dir(QStringLiteral("/sys/class/power_supply"));
    for (const QString &supply : dir.entryList(QDir::Dirs | QDir::NoDotAndDotDot)) {
        const QString path = dir.filePath(supply);
        if (readText(path + QStringLiteral("/type")) == QLatin1String("Battery"))
            return path;
    }
    return {};
}

} // namespace

SystemInfo::SystemInfo(QObject *parent)
    : QObject(parent)
{
    m_os = QSysInfo::prettyProductName();
    m_host = QSysInfo::machineHostName();
    m_kernel = QSysInfo::kernelVersion();
    m_cores = QThread::idealThreadCount();
    for (const QString &line : readText(QStringLiteral("/proc/cpuinfo")).split(u'\n')) {
        if (line.startsWith(QLatin1String("model name"))) {
            m_cpu = shortCpuName(line.section(u':', 1));
            break;
        }
    }
    m_thermalZone = findThermalZone();
    m_batteryDir = findBattery();

    connect(&m_timer, &QTimer::timeout, this, &SystemInfo::refresh);
    m_timer.start(2000);
    refresh();
}

void SystemInfo::refresh()
{
    // CPU: busy time over total time since the last reading.
    const QStringList cpu = readText(QStringLiteral("/proc/stat")).section(u'\n', 0, 0).split(u' ', Qt::SkipEmptyParts);
    if (cpu.size() >= 8) {
        quint64 total = 0;
        for (int i = 1; i < cpu.size(); ++i)
            total += cpu[i].toULongLong();
        // Idle and waiting on I/O are the two that are not work.
        const quint64 busy = total - cpu[4].toULongLong() - cpu[5].toULongLong();
        if (m_total && total > m_total)
            m_cpuLoad = qBound(0.0, qreal(busy - m_busy) / qreal(total - m_total), 1.0);
        m_busy = busy;
        m_total = total;
    }

    // Memory: what is not available to programs is in use.
    qreal total = 0, available = 0;
    for (const QString &line : readText(QStringLiteral("/proc/meminfo")).split(u'\n')) {
        const qreal kib = line.section(u':', 1).simplified().section(u' ', 0, 0).toDouble();
        if (line.startsWith(QLatin1String("MemTotal:")))
            total = kib * 1024;
        else if (line.startsWith(QLatin1String("MemAvailable:")))
            available = kib * 1024;
    }
    m_memoryTotal = total;
    m_memoryUsed = total - available;

    m_uptime = readText(QStringLiteral("/proc/uptime")).section(u' ', 0, 0).toDouble();

    if (!m_thermalZone.isEmpty()) {
        bool ok = false;
        const qreal milli = readText(m_thermalZone + QStringLiteral("/temp")).toDouble(&ok);
        m_temperature = ok ? milli / 1000.0 : -1;
    }

    if (!m_batteryDir.isEmpty()) {
        bool ok = false;
        const int capacity = readText(m_batteryDir + QStringLiteral("/capacity")).toInt(&ok);
        m_battery = ok ? capacity : -1;
        m_charging = readText(m_batteryDir + QStringLiteral("/status")) == QLatin1String("Charging");
    }

    emit changed();
}
