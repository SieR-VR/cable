import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Slider } from "@/components/ui/slider"
import { Badge } from "@/components/ui/badge"
import { Switch } from "@/components/ui/switch"
import type { AudioDevice } from "./audio-multiplexer"
import { Mic, Headphones, Volume2, VolumeX } from "lucide-react"

interface AudioDeviceManagerProps {
  devices: AudioDevice[]
  onDeviceVolumeChange: (deviceId: string, volume: number) => void
  onDeviceToggle: (deviceId: string) => void
  audioLevels?: Map<string, Float32Array>
}

export function AudioDeviceManager({
  devices,
  onDeviceVolumeChange,
  onDeviceToggle,
  audioLevels,
}: AudioDeviceManagerProps) {
  const inputDevices = devices.filter((device) => device.type === "input")
  const outputDevices = devices.filter((device) => device.type === "output")

  const DeviceCard = ({ device }: { device: AudioDevice }) => {
    const deviceLevels = audioLevels?.get(`${device.type}-${device.id}`)
    const averageLevel = deviceLevels
      ? deviceLevels.reduce((sum, val) => sum + Math.max(0, val + 100), 0) / deviceLevels.length
      : 0

    return (
      <Card className="bg-card border-border">
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {device.type === "input" ? (
                <Mic className="h-4 w-4 text-primary" />
              ) : (
                <Headphones className="h-4 w-4 text-primary" />
              )}
              <CardTitle className="text-sm font-medium">{device.name}</CardTitle>
            </div>
            <div className="flex items-center gap-2">
              {device.isDefault && (
                <Badge variant="secondary" className="text-xs">
                  Default
                </Badge>
              )}
              <Switch checked={device.isActive} onCheckedChange={() => onDeviceToggle(device.id)} />
            </div>
          </div>
        </CardHeader>
        <CardContent className="pt-0">
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span>{device.channels} channels</span>
              <span>•</span>
              <span>{device.type === "input" ? "Recording" : "Playback"}</span>
            </div>

            <div className="flex items-center gap-3">
              {device.volume === 0 ? (
                <VolumeX className="h-4 w-4 text-muted-foreground" />
              ) : (
                <Volume2 className="h-4 w-4 text-muted-foreground" />
              )}
              <div className="flex-1">
                <Slider
                  value={[device.volume]}
                  onValueChange={(value) => onDeviceVolumeChange(device.id, value[0])}
                  max={100}
                  step={1}
                  disabled={!device.isActive}
                  className="w-full"
                />
              </div>
              <span className="text-sm text-muted-foreground w-8">{device.volume}%</span>
            </div>

            {device.isActive && (
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary transition-all duration-100"
                  style={{
                    width: `${Math.min(100, averageLevel * 2)}%`,
                    animation: averageLevel > 0 ? "pulse 1s ease-in-out infinite alternate" : "none",
                  }}
                />
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold mb-4 text-foreground">Input Devices</h2>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {inputDevices.map((device) => (
            <DeviceCard key={device.id} device={device} />
          ))}
        </div>
      </div>

      <div>
        <h2 className="text-xl font-semibold mb-4 text-foreground">Output Devices</h2>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {outputDevices.map((device) => (
            <DeviceCard key={device.id} device={device} />
          ))}
        </div>
      </div>
    </div>
  )
}
