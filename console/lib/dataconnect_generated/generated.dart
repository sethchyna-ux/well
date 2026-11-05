library dataconnect_generated;
import 'package:firebase_data_connect/firebase_data_connect.dart';
import 'package:flutter/foundation.dart';
import 'dart:convert';

part 'create_user.dart';

part 'update_user.dart';

part 'delete_user.dart';

part 'get_current_user.dart';

part 'list_all_users.dart';

part 'create_profile.dart';

part 'update_profile.dart';

part 'delete_profile.dart';

part 'get_profile.dart';

part 'list_my_profiles.dart';

part 'create_shell_config.dart';

part 'update_shell_config.dart';

part 'delete_shell_config.dart';

part 'get_shell_config.dart';

part 'list_my_shell_configs.dart';

part 'add_command_history.dart';

part 'delete_command_history.dart';

part 'get_command_history.dart';

part 'list_my_command_history.dart';

part 'create_terminal_theme.dart';

part 'update_terminal_theme.dart';

part 'delete_terminal_theme.dart';

part 'get_terminal_theme.dart';

part 'list_my_terminal_themes.dart';

part 'sync_device.dart';

part 'update_device_sync.dart';

part 'delete_device_sync.dart';

part 'get_device_sync.dart';

part 'list_my_devices.dart';







class ExampleConnector {
  
  
  CreateUserVariablesBuilder createUser () {
    return CreateUserVariablesBuilder(dataConnect, );
  }
  
  
  UpdateUserVariablesBuilder updateUser ({required String username, }) {
    return UpdateUserVariablesBuilder(dataConnect, username: username,);
  }
  
  
  DeleteUserVariablesBuilder deleteUser () {
    return DeleteUserVariablesBuilder(dataConnect, );
  }
  
  
  GetCurrentUserVariablesBuilder getCurrentUser () {
    return GetCurrentUserVariablesBuilder(dataConnect, );
  }
  
  
  ListAllUsersVariablesBuilder listAllUsers () {
    return ListAllUsersVariablesBuilder(dataConnect, );
  }
  
  
  CreateProfileVariablesBuilder createProfile ({required String name, }) {
    return CreateProfileVariablesBuilder(dataConnect, name: name,);
  }
  
  
  UpdateProfileVariablesBuilder updateProfile ({required String id, }) {
    return UpdateProfileVariablesBuilder(dataConnect, id: id,);
  }
  
  
  DeleteProfileVariablesBuilder deleteProfile ({required String id, }) {
    return DeleteProfileVariablesBuilder(dataConnect, id: id,);
  }
  
  
  GetProfileVariablesBuilder getProfile ({required String id, }) {
    return GetProfileVariablesBuilder(dataConnect, id: id,);
  }
  
  
  ListMyProfilesVariablesBuilder listMyProfiles () {
    return ListMyProfilesVariablesBuilder(dataConnect, );
  }
  
  
  CreateShellConfigVariablesBuilder createShellConfig ({required String profileId, required String prompt, required String alias, }) {
    return CreateShellConfigVariablesBuilder(dataConnect, profileId: profileId,prompt: prompt,alias: alias,);
  }
  
  
  UpdateShellConfigVariablesBuilder updateShellConfig ({required String id, }) {
    return UpdateShellConfigVariablesBuilder(dataConnect, id: id,);
  }
  
  
  DeleteShellConfigVariablesBuilder deleteShellConfig ({required String id, }) {
    return DeleteShellConfigVariablesBuilder(dataConnect, id: id,);
  }
  
  
  GetShellConfigVariablesBuilder getShellConfig ({required String id, }) {
    return GetShellConfigVariablesBuilder(dataConnect, id: id,);
  }
  
  
  ListMyShellConfigsVariablesBuilder listMyShellConfigs () {
    return ListMyShellConfigsVariablesBuilder(dataConnect, );
  }
  
  
  AddCommandHistoryVariablesBuilder addCommandHistory ({required String cmd, }) {
    return AddCommandHistoryVariablesBuilder(dataConnect, cmd: cmd,);
  }
  
  
  DeleteCommandHistoryVariablesBuilder deleteCommandHistory ({required String id, }) {
    return DeleteCommandHistoryVariablesBuilder(dataConnect, id: id,);
  }
  
  
  GetCommandHistoryVariablesBuilder getCommandHistory ({required String id, }) {
    return GetCommandHistoryVariablesBuilder(dataConnect, id: id,);
  }
  
  
  ListMyCommandHistoryVariablesBuilder listMyCommandHistory () {
    return ListMyCommandHistoryVariablesBuilder(dataConnect, );
  }
  
  
  CreateTerminalThemeVariablesBuilder createTerminalTheme ({required String name, required String palette, required String profileId, }) {
    return CreateTerminalThemeVariablesBuilder(dataConnect, name: name,palette: palette,profileId: profileId,);
  }
  
  
  UpdateTerminalThemeVariablesBuilder updateTerminalTheme ({required String id, }) {
    return UpdateTerminalThemeVariablesBuilder(dataConnect, id: id,);
  }
  
  
  DeleteTerminalThemeVariablesBuilder deleteTerminalTheme ({required String id, }) {
    return DeleteTerminalThemeVariablesBuilder(dataConnect, id: id,);
  }
  
  
  GetTerminalThemeVariablesBuilder getTerminalTheme ({required String id, }) {
    return GetTerminalThemeVariablesBuilder(dataConnect, id: id,);
  }
  
  
  ListMyTerminalThemesVariablesBuilder listMyTerminalThemes () {
    return ListMyTerminalThemesVariablesBuilder(dataConnect, );
  }
  
  
  SyncDeviceVariablesBuilder syncDevice ({required String name, }) {
    return SyncDeviceVariablesBuilder(dataConnect, name: name,);
  }
  
  
  UpdateDeviceSyncVariablesBuilder updateDeviceSync ({required String id, }) {
    return UpdateDeviceSyncVariablesBuilder(dataConnect, id: id,);
  }
  
  
  DeleteDeviceSyncVariablesBuilder deleteDeviceSync ({required String id, }) {
    return DeleteDeviceSyncVariablesBuilder(dataConnect, id: id,);
  }
  
  
  GetDeviceSyncVariablesBuilder getDeviceSync ({required String id, }) {
    return GetDeviceSyncVariablesBuilder(dataConnect, id: id,);
  }
  
  
  ListMyDevicesVariablesBuilder listMyDevices () {
    return ListMyDevicesVariablesBuilder(dataConnect, );
  }
  

  static ConnectorConfig connectorConfig = ConnectorConfig(
    'us-central1',
    'example',
    'well',
  );

  ExampleConnector({required this.dataConnect});
  static ExampleConnector get instance {
    
    CacheSettings cacheSettings = CacheSettings(
      maxAge: Duration(milliseconds:0),
      storage: CacheStorage.persistent,
    );
    
    return ExampleConnector(
        dataConnect: FirebaseDataConnect.instanceFor(
            connectorConfig: connectorConfig,
            
            cacheSettings: cacheSettings,
            
            sdkType: CallerSDKType.generated));
  }

  FirebaseDataConnect dataConnect;
}
