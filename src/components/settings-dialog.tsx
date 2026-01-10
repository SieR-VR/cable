"use client"

import type React from "react"

import { useState, useEffect } from "react"
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Slider } from "@/components/ui/slider"
import { Switch } from "@/components/ui/switch"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { Settings, Download, Upload, RotateCcw, Save, Trash2, Copy } from "lucide-react"
import type { AudioRoutingEngine } from "@/lib/audio-engine"

interface AudioSettings {
  sampleRate: number
  bufferSize: number
  latency: number
  enableLowLatencyMode: boolean
  enableHighQualityResampling: boolean
  maxInputChannels: number
  maxOutputChannels: number
}

interface UISettings {
  theme: "light" | "dark" | "system"
  compactMode: boolean
  showAdvancedControls: boolean
  enableAnimations: boolean
  autoSaveInterval: number
  showTooltips: boolean
}

interface Preset {
  id: string
  name: string
  description: string
  audioSettings: AudioSettings
  uiSettings: UISettings
  connections: any[]
  effects: any[]
  createdAt: Date
  isDefault: boolean
}

interface SettingsDialogProps {
  audioEngine: AudioRoutingEngine
  children: React.ReactNode
}

export function SettingsDialog({ audioEngine, children }: SettingsDialogProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [audioSettings, setAudioSettings] = useState<AudioSettings>({
    sampleRate: 44100,
    bufferSize: 256,
    latency: 20,
    enableLowLatencyMode: false,
    enableHighQualityResampling: true,
    maxInputChannels: 8,
    maxOutputChannels: 8,
  })

  const [uiSettings, setUISettings] = useState<UISettings>({
    theme: "dark",
    compactMode: false,
    showAdvancedControls: true,
    enableAnimations: true,
    autoSaveInterval: 30,
    showTooltips: true,
  })

  const [presets, setPresets] = useState<Preset[]>([])
  const [selectedPreset, setSelectedPreset] = useState<string | null>(null)
  const [newPresetName, setNewPresetName] = useState("")
  const [newPresetDescription, setNewPresetDescription] = useState("")

  // Load settings from localStorage on mount
  useEffect(() => {
    const savedAudioSettings = localStorage.getItem("audioMultiplexer_audioSettings")
    const savedUISettings = localStorage.getItem("audioMultiplexer_uiSettings")
    const savedPresets = localStorage.getItem("audioMultiplexer_presets")

    if (savedAudioSettings) {
      setAudioSettings(JSON.parse(savedAudioSettings))
    }
    if (savedUISettings) {
      setUISettings(JSON.parse(savedUISettings))
    }
    if (savedPresets) {
      setPresets(JSON.parse(savedPresets))
    } else {
      // Create default presets
      const defaultPresets = createDefaultPresets()
      setPresets(defaultPresets)
      localStorage.setItem("audioMultiplexer_presets", JSON.stringify(defaultPresets))
    }
  }, [])

  // Save settings to localStorage when changed
  useEffect(() => {
    localStorage.setItem("audioMultiplexer_audioSettings", JSON.stringify(audioSettings))
  }, [audioSettings])

  useEffect(() => {
    localStorage.setItem("audioMultiplexer_uiSettings", JSON.stringify(uiSettings))
  }, [uiSettings])

  const createDefaultPresets = (): Preset[] => [
    {
      id: "default-studio",
      name: "Studio Recording",
      description: "Optimized for professional studio recording with low latency",
      audioSettings: {
        ...audioSettings,
        sampleRate: 48000,
        bufferSize: 128,
        latency: 10,
        enableLowLatencyMode: true,
      },
      uiSettings,
      connections: [],
      effects: [],
      createdAt: new Date(),
      isDefault: true,
    },
    {
      id: "default-streaming",
      name: "Live Streaming",
      description: "Balanced settings for live streaming and content creation",
      audioSettings: {
        ...audioSettings,
        sampleRate: 44100,
        bufferSize: 512,
        latency: 30,
        enableLowLatencyMode: false,
      },
      uiSettings,
      connections: [],
      effects: [],
      createdAt: new Date(),
      isDefault: true,
    },
    {
      id: "default-podcast",
      name: "Podcast Production",
      description: "Settings optimized for podcast recording and editing",
      audioSettings: {
        ...audioSettings,
        sampleRate: 44100,
        bufferSize: 256,
        latency: 20,
        enableHighQualityResampling: true,
      },
      uiSettings,
      connections: [],
      effects: [],
      createdAt: new Date(),
      isDefault: true,
    },
  ]

  const savePreset = () => {
    if (!newPresetName.trim()) return

    const newPreset: Preset = {
      id: `preset-${Date.now()}`,
      name: newPresetName,
      description: newPresetDescription,
      audioSettings,
      uiSettings,
      connections: [], // Would be populated with current connections
      effects: [], // Would be populated with current effects
      createdAt: new Date(),
      isDefault: false,
    }

    const updatedPresets = [...presets, newPreset]
    setPresets(updatedPresets)
    localStorage.setItem("audioMultiplexer_presets", JSON.stringify(updatedPresets))

    setNewPresetName("")
    setNewPresetDescription("")
  }

  const loadPreset = (presetId: string) => {
    const preset = presets.find((p) => p.id === presetId)
    if (!preset) return

    setAudioSettings(preset.audioSettings)
    setUISettings(preset.uiSettings)
    setSelectedPreset(presetId)

    // Apply settings to audio engine
    console.log("[v0] Loaded preset:", preset.name)
  }

  const deletePreset = (presetId: string) => {
    const updatedPresets = presets.filter((p) => p.id !== presetId && !p.isDefault)
    setPresets(updatedPresets)
    localStorage.setItem("audioMultiplexer_presets", JSON.stringify(updatedPresets))

    if (selectedPreset === presetId) {
      setSelectedPreset(null)
    }
  }

  const duplicatePreset = (presetId: string) => {
    const preset = presets.find((p) => p.id === presetId)
    if (!preset) return

    const duplicatedPreset: Preset = {
      ...preset,
      id: `preset-${Date.now()}`,
      name: `${preset.name} (Copy)`,
      createdAt: new Date(),
      isDefault: false,
    }

    const updatedPresets = [...presets, duplicatedPreset]
    setPresets(updatedPresets)
    localStorage.setItem("audioMultiplexer_presets", JSON.stringify(updatedPresets))
  }

  const exportSettings = () => {
    const exportData = {
      audioSettings,
      uiSettings,
      presets: presets.filter((p) => !p.isDefault),
      exportedAt: new Date(),
      version: "1.0",
    }

    const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: "application/json" })
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url
    a.download = `audio-multiplexer-settings-${new Date().toISOString().split("T")[0]}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  }

  const importSettings = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (!file) return

    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const importData = JSON.parse(e.target?.result as string)

        if (importData.audioSettings) {
          setAudioSettings(importData.audioSettings)
        }
        if (importData.uiSettings) {
          setUISettings(importData.uiSettings)
        }
        if (importData.presets) {
          const importedPresets = importData.presets.map((p: any) => ({
            ...p,
            id: `imported-${Date.now()}-${Math.random()}`,
            createdAt: new Date(p.createdAt),
            isDefault: false,
          }))
          const updatedPresets = [...presets, ...importedPresets]
          setPresets(updatedPresets)
          localStorage.setItem("audioMultiplexer_presets", JSON.stringify(updatedPresets))
        }

        console.log("[v0] Settings imported successfully")
      } catch (error) {
        console.error("[v0] Failed to import settings:", error)
      }
    }
    reader.readAsText(file)
  }

  const resetToDefaults = () => {
    const defaultAudioSettings: AudioSettings = {
      sampleRate: 44100,
      bufferSize: 256,
      latency: 20,
      enableLowLatencyMode: false,
      enableHighQualityResampling: true,
      maxInputChannels: 8,
      maxOutputChannels: 8,
    }

    const defaultUISettings: UISettings = {
      theme: "dark",
      compactMode: false,
      showAdvancedControls: true,
      enableAnimations: true,
      autoSaveInterval: 30,
      showTooltips: true,
    }

    setAudioSettings(defaultAudioSettings)
    setUISettings(defaultUISettings)
    setSelectedPreset(null)
  }

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogTrigger asChild>{children}</DialogTrigger>
      <DialogContent className="max-w-4xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Settings className="h-5 w-5" />
            Audio Multiplexer Settings
          </DialogTitle>
        </DialogHeader>

        <Tabs defaultValue="audio" className="w-full">
          <TabsList className="grid w-full grid-cols-4">
            <TabsTrigger value="audio">Audio</TabsTrigger>
            <TabsTrigger value="interface">Interface</TabsTrigger>
            <TabsTrigger value="presets">Presets</TabsTrigger>
            <TabsTrigger value="advanced">Advanced</TabsTrigger>
          </TabsList>

          <TabsContent value="audio" className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle className="text-lg">Audio Engine Settings</CardTitle>
              </CardHeader>
              <CardContent className="space-y-6">
                <div className="grid grid-cols-2 gap-6">
                  <div className="space-y-2">
                    <Label>Sample Rate</Label>
                    <Select
                      value={audioSettings.sampleRate.toString()}
                      onValueChange={(value) =>
                        setAudioSettings({ ...audioSettings, sampleRate: Number.parseInt(value) })
                      }
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="44100">44.1 kHz</SelectItem>
                        <SelectItem value="48000">48 kHz</SelectItem>
                        <SelectItem value="88200">88.2 kHz</SelectItem>
                        <SelectItem value="96000">96 kHz</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="space-y-2">
                    <Label>Buffer Size</Label>
                    <Select
                      value={audioSettings.bufferSize.toString()}
                      onValueChange={(value) =>
                        setAudioSettings({ ...audioSettings, bufferSize: Number.parseInt(value) })
                      }
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="64">64 samples</SelectItem>
                        <SelectItem value="128">128 samples</SelectItem>
                        <SelectItem value="256">256 samples</SelectItem>
                        <SelectItem value="512">512 samples</SelectItem>
                        <SelectItem value="1024">1024 samples</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <div className="space-y-2">
                  <Label>Target Latency: {audioSettings.latency}ms</Label>
                  <Slider
                    value={[audioSettings.latency]}
                    onValueChange={(value) => setAudioSettings({ ...audioSettings, latency: value[0] })}
                    max={100}
                    min={5}
                    step={1}
                  />
                </div>

                <Separator />

                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div>
                      <Label>Low Latency Mode</Label>
                      <p className="text-xs text-muted-foreground">Optimize for minimal audio delay</p>
                    </div>
                    <Switch
                      checked={audioSettings.enableLowLatencyMode}
                      onCheckedChange={(checked) =>
                        setAudioSettings({ ...audioSettings, enableLowLatencyMode: checked })
                      }
                    />
                  </div>

                  <div className="flex items-center justify-between">
                    <div>
                      <Label>High Quality Resampling</Label>
                      <p className="text-xs text-muted-foreground">
                        Use advanced algorithms for sample rate conversion
                      </p>
                    </div>
                    <Switch
                      checked={audioSettings.enableHighQualityResampling}
                      onCheckedChange={(checked) =>
                        setAudioSettings({ ...audioSettings, enableHighQualityResampling: checked })
                      }
                    />
                  </div>
                </div>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="interface" className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle className="text-lg">User Interface Settings</CardTitle>
              </CardHeader>
              <CardContent className="space-y-6">
                <div className="space-y-2">
                  <Label>Theme</Label>
                  <Select
                    value={uiSettings.theme}
                    onValueChange={(value: any) => setUISettings({ ...uiSettings, theme: value })}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="light">Light</SelectItem>
                      <SelectItem value="dark">Dark</SelectItem>
                      <SelectItem value="system">System</SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                <div className="space-y-2">
                  <Label>Auto-save Interval: {uiSettings.autoSaveInterval} seconds</Label>
                  <Slider
                    value={[uiSettings.autoSaveInterval]}
                    onValueChange={(value) => setUISettings({ ...uiSettings, autoSaveInterval: value[0] })}
                    max={300}
                    min={10}
                    step={10}
                  />
                </div>

                <Separator />

                <div className="space-y-4">
                  {[
                    { key: "compactMode", label: "Compact Mode", description: "Use smaller UI elements" },
                    {
                      key: "showAdvancedControls",
                      label: "Advanced Controls",
                      description: "Show professional audio controls",
                    },
                    {
                      key: "enableAnimations",
                      label: "Animations",
                      description: "Enable UI animations and transitions",
                    },
                    { key: "showTooltips", label: "Tooltips", description: "Show helpful tooltips on hover" },
                  ].map((setting) => (
                    <div key={setting.key} className="flex items-center justify-between">
                      <div>
                        <Label>{setting.label}</Label>
                        <p className="text-xs text-muted-foreground">{setting.description}</p>
                      </div>
                      <Switch
                        checked={uiSettings[setting.key as keyof UISettings] as boolean}
                        onCheckedChange={(checked) => setUISettings({ ...uiSettings, [setting.key]: checked })}
                      />
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="presets" className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-semibold">Configuration Presets</h3>
              <div className="flex items-center gap-2">
                <Button variant="outline" size="sm" onClick={exportSettings}>
                  <Download className="h-4 w-4 mr-2" />
                  Export
                </Button>
                <Button variant="outline" size="sm" asChild>
                  <label>
                    <Upload className="h-4 w-4 mr-2" />
                    Import
                    <input type="file" accept=".json" onChange={importSettings} className="hidden" />
                  </label>
                </Button>
              </div>
            </div>

            <Card>
              <CardHeader>
                <CardTitle className="text-base">Create New Preset</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label>Preset Name</Label>
                    <Input
                      value={newPresetName}
                      onChange={(e) => setNewPresetName(e.target.value)}
                      placeholder="My Custom Preset"
                    />
                  </div>
                  <div className="space-y-2">
                    <Label>Description</Label>
                    <Input
                      value={newPresetDescription}
                      onChange={(e) => setNewPresetDescription(e.target.value)}
                      placeholder="Description of this preset"
                    />
                  </div>
                </div>
                <Button onClick={savePreset} disabled={!newPresetName.trim()} className="w-full">
                  <Save className="h-4 w-4 mr-2" />
                  Save Current Configuration
                </Button>
              </CardContent>
            </Card>

            <div className="grid gap-3">
              {presets.map((preset) => (
                <Card key={preset.id} className={selectedPreset === preset.id ? "ring-2 ring-primary" : ""}>
                  <CardContent className="p-4">
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <div className="flex items-center gap-2">
                          <h4 className="font-medium">{preset.name}</h4>
                          {preset.isDefault && (
                            <Badge variant="secondary" className="text-xs">
                              Default
                            </Badge>
                          )}
                        </div>
                        <p className="text-sm text-muted-foreground">{preset.description}</p>
                        <div className="flex items-center gap-4 mt-2 text-xs text-muted-foreground">
                          <span>{preset.audioSettings.sampleRate / 1000}kHz</span>
                          <span>{preset.audioSettings.bufferSize} samples</span>
                          <span>{preset.audioSettings.latency}ms latency</span>
                        </div>
                      </div>
                      <div className="flex items-center gap-1">
                        <Button variant="ghost" size="sm" onClick={() => loadPreset(preset.id)}>
                          Load
                        </Button>
                        <Button variant="ghost" size="sm" onClick={() => duplicatePreset(preset.id)}>
                          <Copy className="h-3 w-3" />
                        </Button>
                        {!preset.isDefault && (
                          <Button variant="ghost" size="sm" onClick={() => deletePreset(preset.id)}>
                            <Trash2 className="h-3 w-3" />
                          </Button>
                        )}
                      </div>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          </TabsContent>

          <TabsContent value="advanced" className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle className="text-lg">Advanced Settings</CardTitle>
              </CardHeader>
              <CardContent className="space-y-6">
                <div className="grid grid-cols-2 gap-6">
                  <div className="space-y-2">
                    <Label>Max Input Channels: {audioSettings.maxInputChannels}</Label>
                    <Slider
                      value={[audioSettings.maxInputChannels]}
                      onValueChange={(value) => setAudioSettings({ ...audioSettings, maxInputChannels: value[0] })}
                      max={32}
                      min={2}
                      step={2}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label>Max Output Channels: {audioSettings.maxOutputChannels}</Label>
                    <Slider
                      value={[audioSettings.maxOutputChannels]}
                      onValueChange={(value) => setAudioSettings({ ...audioSettings, maxOutputChannels: value[0] })}
                      max={32}
                      min={2}
                      step={2}
                    />
                  </div>
                </div>

                <Separator />

                <div className="space-y-4">
                  <h4 className="font-medium">System Information</h4>
                  <div className="grid grid-cols-2 gap-4 text-sm">
                    <div>
                      <span className="text-muted-foreground">Audio Context State:</span>
                      <span className="ml-2">{audioEngine.getStatus().contextState || "Not initialized"}</span>
                    </div>
                    <div>
                      <span className="text-muted-foreground">Sample Rate:</span>
                      <span className="ml-2">{audioEngine.getStatus().sampleRate || "Unknown"} Hz</span>
                    </div>
                    <div>
                      <span className="text-muted-foreground">Active Nodes:</span>
                      <span className="ml-2">
                        {audioEngine.getStatus().inputNodes}I / {audioEngine.getStatus().outputNodes}O
                      </span>
                    </div>
                    <div>
                      <span className="text-muted-foreground">Active Routes:</span>
                      <span className="ml-2">{audioEngine.getStatus().activeRoutes}</span>
                    </div>
                  </div>
                </div>

                <Separator />

                <div className="flex items-center justify-between">
                  <div>
                    <h4 className="font-medium">Reset to Defaults</h4>
                    <p className="text-sm text-muted-foreground">Restore all settings to their default values</p>
                  </div>
                  <Button variant="destructive" onClick={resetToDefaults}>
                    <RotateCcw className="h-4 w-4 mr-2" />
                    Reset
                  </Button>
                </div>
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  )
}
