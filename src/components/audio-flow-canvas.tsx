import type React from "react";

import { useState, useRef, useCallback, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { AudioDevice, AudioConnection } from "./audio-multiplexer";
import { Mic, Headphones, Volume2, VolumeX, Link, Unlink } from "lucide-react";

interface AudioFlowCanvasProps {
  devices: AudioDevice[];
  connections: AudioConnection[];
  onConnectionChange: (connections: AudioConnection[]) => void;
  onDeviceVolumeChange: (deviceId: string, volume: number) => void;
  isProcessing: boolean;
  audioLevels?: Map<string, Float32Array>;
}

interface NodePosition {
  x: number;
  y: number;
}

interface DragState {
  isDragging: boolean;
  dragType: "node" | "connection" | null;
  sourceNodeId: string | null;
  startDragPosition: { x: number; y: number };
  startNodePosition: { x: number; y: number };
}

export function AudioFlowCanvas({
  devices,
  connections,
  onConnectionChange,
  onDeviceVolumeChange,
  isProcessing,
  audioLevels,
}: AudioFlowCanvasProps) {
  const canvasRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const [nodePositions, setNodePositions] = useState<
    Record<string, NodePosition>
  >({});
  const [dragState, setDragState] = useState<DragState>({
    isDragging: false,
    dragType: null,
    sourceNodeId: null,
    startDragPosition: { x: 0, y: 0 },
    startNodePosition: { x: 0, y: 0 },
  });
  const [tempConnection, setTempConnection] = useState<{
    start: NodePosition;
    end: NodePosition;
  } | null>(null);

  // Initialize node positions
  useEffect(() => {
    const inputDevices = devices.filter((d) => d.type === "input");
    const outputDevices = devices.filter((d) => d.type === "output");
    const positions: Record<string, NodePosition> = {};

    const canvasWidth = canvasRef.current?.clientWidth;

    // Position input devices on the left
    inputDevices.forEach((device, index) => {
      positions[device.id] = {
        x: 100,
        y: 100 + index * 150,
      };
    });

    // Position output devices on the right
    outputDevices.forEach((device, index) => {
      positions[device.id] = {
        x: (canvasWidth ?? 600) - 100 - 75,
        y: 100 + index * 150,
      };
    });

    setNodePositions(positions);
  }, [devices]);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent, nodeId: string, type: "node" | "connection") => {
      e.preventDefault();
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;

      setDragState({
        isDragging: true,
        dragType: type,
        sourceNodeId: nodeId,
        startDragPosition: {
          x: e.clientX - rect.left,
          y: e.clientY - rect.top,
        },
        startNodePosition: nodePositions[nodeId],
      });

      if (type === "connection") {
        const nodePos = nodePositions[nodeId];
        if (nodePos) {
          setTempConnection({
            start: nodePos,
            end: { x: e.clientX - rect.left, y: e.clientY - rect.top },
          });
        }
      }
    },
    [nodePositions]
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!dragState.isDragging || !canvasRef.current) return;

      const rect = canvasRef.current.getBoundingClientRect();
      const currentPos = {
        x: e.clientX - rect.left,
        y: e.clientY - rect.top,
      };

      const targetPos = {
        x:
          dragState.startNodePosition.x -
          dragState.startDragPosition.x +
          currentPos.x,
        y:
          dragState.startNodePosition.y -
          dragState.startDragPosition.y +
          currentPos.y,
      };

      if (dragState.dragType === "node" && dragState.sourceNodeId) {
        setNodePositions((prev) => ({
          ...prev,
          [dragState.sourceNodeId!]: targetPos,
        }));
      } else if (dragState.dragType === "connection" && tempConnection) {
        setTempConnection({
          ...tempConnection,
          end: currentPos,
        });
      }
    },
    [dragState, tempConnection]
  );

  const handleMouseUp = useCallback(
    (_: React.MouseEvent, targetNodeId?: string) => {
      if (
        dragState.dragType === "connection" &&
        dragState.sourceNodeId &&
        targetNodeId
      ) {
        const sourceDevice = devices.find(
          (d) => d.id === dragState.sourceNodeId
        );
        const targetDevice = devices.find((d) => d.id === targetNodeId);

        if (
          sourceDevice &&
          targetDevice &&
          sourceDevice.type !== targetDevice.type
        ) {
          const newConnection: AudioConnection = {
            id: `${dragState.sourceNodeId}-${targetNodeId}`,
            sourceId: dragState.sourceNodeId,
            targetId: targetNodeId,
            volume: 75,
            isActive: true,
          };

          const existingConnectionIndex = connections.findIndex(
            (c) =>
              c.sourceId === dragState.sourceNodeId &&
              c.targetId === targetNodeId
          );

          if (existingConnectionIndex === -1) {
            onConnectionChange([...connections, newConnection]);
          }
        }
      }

      setDragState({
        isDragging: false,
        dragType: null,
        sourceNodeId: null,
        startDragPosition: { x: 0, y: 0 },
        startNodePosition: { x: 0, y: 0 },
      });
      setTempConnection(null);
    },
    [dragState, devices, connections, onConnectionChange]
  );

  const removeConnection = (connectionId: string) => {
    onConnectionChange(connections.filter((c) => c.id !== connectionId));
  };

  const updateConnectionVolume = (connectionId: string, volume: number) => {
    onConnectionChange(
      connections.map((c) => (c.id === connectionId ? { ...c, volume } : c))
    );
  };

  const getConnectionPath = (sourceId: string, targetId: string) => {
    const sourcePos = nodePositions[sourceId];
    const targetPos = nodePositions[targetId];
    if (!sourcePos || !targetPos) return "";

    const sourceDevice = devices.find((d) => d.id === sourceId);
    const targetDevice = devices.find((d) => d.id === targetId);

    // Calculate actual connection point positions
    const startX =
      sourceDevice?.type === "input" ? sourcePos.x + 96 : sourcePos.x - 96; // Right side for input, left side for output
    const startY = sourcePos.y + 20; // Connection point vertical position
    const endX =
      targetDevice?.type === "output" ? targetPos.x - 96 : targetPos.x + 96; // Left side for output, right side for input
    const endY = targetPos.y + 20;
    const midX = (startX + endX) / 2;

    return `M ${startX} ${startY} C ${midX} ${startY} ${midX} ${endY} ${endX} ${endY}`;
  };

  const AudioNode = ({ device }: { device: AudioDevice }) => {
    const position = nodePositions[device.id] || { x: 0, y: 0 };
    const isInput = device.type === "input";

    const deviceLevels = audioLevels?.get(`${device.type}-${device.id}`);
    const averageLevel = deviceLevels
      ? deviceLevels.reduce((sum, val) => sum + Math.max(0, val + 100), 0) /
        deviceLevels.length
      : 0;

    return (
      <div
        className="absolute select-none"
        style={{
          left: position.x,
          top: position.y,
          transform: "translate(-50%, -50%)",
        }}
        onMouseUp={(e) => handleMouseUp(e, device.id)}
      >
        <Card
          className="w-48 h-32 bg-card border-border shadow-lg hover:shadow-xl transition-shadow cursor-move"
          onMouseDown={(e) => handleMouseDown(e, device.id, "node")}
        >
          <CardContent className="p-3 h-full flex flex-col justify-between">
            <div className="flex items-center gap-2 min-h-[20px]">
              <div className="flex-shrink-0">
                {isInput ? (
                  <Mic className="h-4 w-4 text-primary" />
                ) : (
                  <Headphones className="h-4 w-4 text-primary" />
                )}
              </div>
              <span className="text-sm font-medium truncate flex-1">
                {device.name}
              </span>
              {device.isDefault && (
                <Badge variant="secondary" className="text-xs flex-shrink-0">
                  Default
                </Badge>
              )}
            </div>

            <div className="flex items-center justify-between relative">
              {isInput && (
                <div
                  className="absolute -right-3 top-1/2 transform -translate-y-1/2 w-4 h-4 rounded-full bg-primary cursor-crosshair hover:bg-primary/80 transition-colors z-10"
                  onMouseDown={(e) => {
                    e.stopPropagation();
                    handleMouseDown(e, device.id, "connection");
                  }}
                  title="Drag to connect"
                />
              )}
              {!isInput && (
                <div
                  className="absolute -left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 rounded-full bg-primary cursor-crosshair hover:bg-primary/80 transition-colors z-10"
                  onMouseDown={(e) => {
                    e.stopPropagation();
                    handleMouseDown(e, device.id, "connection");
                  }}
                  title="Drag to connect"
                />
              )}
            </div>

            {/* Volume Control */}
            <div className="flex items-center gap-2">
              <div className="flex-shrink-0">
                {device.volume === 0 ? (
                  <VolumeX className="h-3 w-3 text-muted-foreground" />
                ) : (
                  <Volume2 className="h-3 w-3 text-muted-foreground" />
                )}
              </div>
              <div className="flex-1 min-w-0">
                <Slider
                  value={[device.volume]}
                  onValueChange={(value) =>
                    onDeviceVolumeChange(device.id, value[0])
                  }
                  max={100}
                  step={1}
                  className="w-full"
                />
              </div>
              <span className="text-xs text-muted-foreground w-8 flex-shrink-0">
                {device.volume}%
              </span>
            </div>

            {device.isActive && isProcessing && (
              <div className="h-1 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary transition-all duration-100"
                  style={{
                    width: `${Math.min(100, averageLevel * 2)}%`,
                    animation:
                      averageLevel > 0
                        ? "pulse 0.5s ease-in-out infinite alternate"
                        : "none",
                  }}
                />
              </div>
            )}

            {/* Status */}
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span className="flex-shrink-0">{device.channels}ch</span>
              <Badge
                variant={device.isActive ? "default" : "secondary"}
                className="text-xs flex-shrink-0"
              >
                {device.isActive ? "Active" : "Inactive"}
              </Badge>
            </div>
          </CardContent>
        </Card>
      </div>
    );
  };

  return (
    <div className="relative h-full bg-muted/20 rounded-lg overflow-hidden">
      <div
        ref={canvasRef}
        className="relative w-full h-full cursor-default"
        onMouseMove={handleMouseMove}
        onMouseUp={(e) => handleMouseUp(e)}
      >
        {/* SVG for connections */}
        <svg
          ref={svgRef}
          className="absolute inset-0 w-full h-full pointer-events-none"
          style={{ zIndex: 1 }}
        >
          <defs>
            <marker
              id="arrowhead"
              markerWidth="10"
              markerHeight="7"
              refX="9"
              refY="3.5"
              orient="auto"
            >
              <polygon
                points="0 0, 10 3.5, 0 7"
                fill="currentColor"
                className="text-primary"
              />
            </marker>
          </defs>

          {/* Existing connections */}
          {connections.map((connection) => (
            <g key={connection.id}>
              <path
                d={getConnectionPath(connection.sourceId, connection.targetId)}
                stroke="currentColor"
                strokeWidth="3"
                fill="none"
                markerEnd="url(#arrowhead)"
                className={`${
                  connection.isActive ? "text-primary" : "text-muted-foreground"
                } transition-colors`}
                style={{
                  opacity: connection.volume / 100,
                }}
              />
            </g>
          ))}

          {tempConnection && dragState.sourceNodeId && (
            <path
              d={(() => {
                const sourceDevice = devices.find(
                  (d) => d.id === dragState.sourceNodeId
                );
                const startX =
                  sourceDevice?.type === "input"
                    ? tempConnection.start.x + 96
                    : tempConnection.start.x - 96;
                const startY = tempConnection.start.y + 20;
                return `M ${startX} ${startY} L ${tempConnection.end.x} ${tempConnection.end.y}`;
              })()}
              stroke="currentColor"
              strokeWidth="2"
              strokeDasharray="5,5"
              fill="none"
              className="text-primary opacity-60"
            />
          )}
        </svg>

        {/* Audio nodes */}
        <div style={{ zIndex: 2 }} className="relative">
          {devices.map((device) => (
            <AudioNode key={device.id} device={device} />
          ))}
        </div>

        {/* Connection controls */}
        {connections.length > 0 && (
          <div
            className="absolute top-4 right-4 space-y-2"
            style={{ zIndex: 3 }}
          >
            <Card className="bg-card/90 backdrop-blur-sm">
              <CardContent className="p-3">
                <h3 className="text-sm font-medium mb-2">Active Connections</h3>
                <div className="space-y-2">
                  {connections.map((connection) => {
                    const sourceDevice = devices.find(
                      (d) => d.id === connection.sourceId
                    );
                    const targetDevice = devices.find(
                      (d) => d.id === connection.targetId
                    );
                    return (
                      <div
                        key={connection.id}
                        className="flex items-center gap-2 text-xs"
                      >
                        <div className="flex items-center gap-1 flex-1">
                          <span className="truncate max-w-16">
                            {sourceDevice?.name}
                          </span>
                          <Link className="h-3 w-3" />
                          <span className="truncate max-w-16">
                            {targetDevice?.name}
                          </span>
                        </div>
                        <div className="w-12">
                          <Slider
                            value={[connection.volume]}
                            onValueChange={(value) =>
                              updateConnectionVolume(connection.id, value[0])
                            }
                            max={100}
                            step={1}
                            className="w-full"
                          />
                        </div>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => removeConnection(connection.id)}
                          className="h-6 w-6 p-0"
                        >
                          <Unlink className="h-3 w-3" />
                        </Button>
                      </div>
                    );
                  })}
                </div>
              </CardContent>
            </Card>
          </div>
        )}

        {/* Instructions */}
        {connections.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
            <Card className="bg-card/80 backdrop-blur-sm">
              <CardContent className="p-6 text-center">
                <h3 className="text-lg font-medium mb-2">
                  Create Audio Connections
                </h3>
                <p className="text-sm text-muted-foreground mb-4">
                  Drag from the connection points on input devices to output
                  devices
                </p>
                <div className="flex items-center justify-center gap-4 text-xs text-muted-foreground">
                  <div className="flex items-center gap-1">
                    <div className="w-3 h-3 rounded-full bg-primary" />
                    <span>Connection Point</span>
                  </div>
                  <div className="flex items-center gap-1">
                    <Link className="h-3 w-3" />
                    <span>Drag to Connect</span>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>
        )}
      </div>
    </div>
  );
}
