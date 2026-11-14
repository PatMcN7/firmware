-- DAQApp read-only vehicle telemetry. No inputs or physics values are modified.
local M = {}
local udp
local elapsed = 0
local accumulator = 0
local sequence = 0
local interval = 1 / 30
local nextErrorLog = 0

local function stop()
  if udp then udp:close() end
  udp = nil
end

local function reset()
  elapsed = 0
  accumulator = 0
  sequence = 0
  nextErrorLog = 0
end

local function start(port, rate)
  port = tonumber(port) or 4445
  rate = tonumber(rate) or 30
  if port ~= math.floor(port) or port < 1 or port > 65535 then return false end
  if rate < 1 or rate > 120 then return false end
  stop()
  local err
  udp, err = socket.udp()
  if not udp then
    log('E', 'daqAppTelemetry', tostring(err))
    return false
  end
  udp:settimeout(0)
  -- sendto, rather than a connected UDP socket: DAQApp may start/stop at any time.
  M.port = port
  interval = 1 / rate
  reset()
  return true
end

local function put(values, key, value)
  -- Missing or non-finite data stays missing, never silently becomes zero.
  if type(value) == 'number' and value == value and math.abs(value) < math.huge then
    values[key] = value
  end
end

local function collect()
  local values = {}
  local e = electrics.values
  local velocity = obj:getVelocity()
  put(values, 'speed_mps', velocity:length())
  put(values, 'wheel_speed_mps', e.wheelspeed)
  put(values, 'rpm', e.rpm)
  put(values, 'gear', e.gearIndex)
  put(values, 'throttle', e.throttle)
  put(values, 'brake', e.brake)
  put(values, 'clutch', e.clutch)
  put(values, 'steering', e.steering_input)
  put(values, 'parking_brake', e.parkingbrake)
  put(values, 'fuel_fraction', e.fuel)
  put(values, 'coolant_temp_c', e.watertemp)
  put(values, 'oil_temp_c', e.oiltemp)
  put(values, 'accel_x_mps2', sensors.gx)
  put(values, 'accel_y_mps2', sensors.gy)
  put(values, 'accel_z_mps2', sensors.gz)
  local x, y, z = obj:getPositionXYZ()
  put(values, 'position_x_m', x)
  put(values, 'position_y_m', y)
  put(values, 'position_z_m', z)
  put(values, 'velocity_x_mps', velocity.x)
  put(values, 'velocity_y_mps', velocity.y)
  put(values, 'velocity_z_mps', velocity.z)
  local roll, pitch, yaw = obj:getRollPitchYaw()
  put(values, 'roll_rad', roll)
  put(values, 'pitch_rad', pitch)
  put(values, 'yaw_rad', yaw)

  -- Only explicit corner names are mapped: do not guess on multi-axle/custom cars.
  for _, wheel in pairs(wheels.wheels or {}) do
    local name = wheel.name
    if name == 'FL' or name == 'FR' or name == 'RL' or name == 'RR' then
      local prefix = 'wheel_' .. name .. '_'
      put(values, prefix .. 'speed_mps', wheel.wheelSpeed)
      put(values, prefix .. 'angular_velocity', wheel.angularVelocity)
      put(values, prefix .. 'load_n', wheel.downForce)
      put(values, prefix .. 'slip_mps', wheel.lastSlip)
      put(values, prefix .. 'side_slip_mps', wheel.lastSideSlip)
      put(values, prefix .. 'drive_torque_nm', wheel.propulsionTorque)
      put(values, prefix .. 'brake_temp_c', wheel.brakeSurfaceTemperature)
      put(values, prefix .. 'brake_core_temp_c', wheel.brakeCoreTemperature)
    end
  end
  return values
end

local function updateGFX(dt)
  if not udp or dt <= 0 then return end
  elapsed = elapsed + dt
  if not playerInfo.firstPlayerSeated then return end
  accumulator = accumulator + dt
  if accumulator < interval then return end
  accumulator = accumulator % interval -- never burst to catch up after a slow frame
  sequence = (sequence + 1) % 4294967296
  local packet = {
    protocol = 'daqapp.beamng', version = 1,
    vehicle_id = obj:getID(), sequence = sequence, sim_time = elapsed,
    signals = collect()
  }
  local sent, err = udp:sendto(jsonEncode(packet), '127.0.0.1', M.port)
  if not sent and elapsed >= nextErrorLog then
    log('W', 'daqAppTelemetry', 'UDP send failed: ' .. tostring(err))
    nextErrorLog = elapsed + 5
  end
end

M.start = start
M.stop = stop
M.onReset = reset
M.onExtensionUnloaded = stop
M.updateGFX = updateGFX
return M
