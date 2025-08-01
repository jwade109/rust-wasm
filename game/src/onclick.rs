use crate::scenes::{CursorMode, ThrottleLevel};
use crate::sim_rate::SimRate;
use starling::prelude::*;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnClick {
    Orbiter(EntityId),
    Exit,
    Save,
    Load,
    ToggleDrawMode,
    ClearTracks,
    CreateGroup,
    DisbandGroup(EntityId),
    ClearOrbits,
    CurrentBody(EntityId),
    SelectedCount,
    AutopilotingCount,
    PilotOrbiter,
    Group(EntityId),
    TogglePause,
    World,
    SimSpeed(SimRate),
    GlobalOrbit(usize),
    DeleteOrbit(usize),
    DeleteOrbiter,
    ClearMission,
    CommitMission,
    CursorMode(CursorMode),
    GoToScene(usize),
    ThrottleLevel(ThrottleLevel),
    SetTarget(EntityId),
    SetPilot(EntityId),
    ClearTarget,
    ClearPilot,
    SwapOwnshipTarget,
    PinObject(EntityId),
    UnpinObject(EntityId),
    SelectPart(String),
    ToggleLayer(PartLayer),
    LoadVehicle(PathBuf),
    DismissExitDialog,
    ConfirmExitDialog,
    TogglePartsMenuCollapsed,
    ToggleVehiclesMenuCollapsed,
    ToggleLayersMenuCollapsed,
    ToggleVehicleInfo,
    SendToSurface,
    IncrementThrottle(i32),
    OpenNewCraft,
    WriteVehicleToImage,
    RotateCraft,
    NormalizeCraft,
    ToggleThruster(usize),
    ReloadGame,
    IncreaseGravity,
    DecreaseGravity,
    IncreaseWind,
    DecreaseWind,
    ToggleSurfaceSleep,
    SetRecipe(PartId, RecipeListing),
    ClearContents(PartId),
    GoToSurface(EntityId),
    Nullopt,
}
