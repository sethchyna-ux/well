part of 'generated.dart';

class CreateTerminalThemeVariablesBuilder {
  String name;
  String palette;
  String profileId;

  final FirebaseDataConnect _dataConnect;
  CreateTerminalThemeVariablesBuilder(this._dataConnect, {required  this.name,required  this.palette,required  this.profileId,});
  Deserializer<CreateTerminalThemeData> dataDeserializer = (dynamic json)  => CreateTerminalThemeData.fromJson(jsonDecode(json));
  Serializer<CreateTerminalThemeVariables> varsSerializer = (CreateTerminalThemeVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<CreateTerminalThemeData, CreateTerminalThemeVariables>> execute() {
    return ref().execute();
  }

  MutationRef<CreateTerminalThemeData, CreateTerminalThemeVariables> ref() {
    CreateTerminalThemeVariables vars= CreateTerminalThemeVariables(name: name,palette: palette,profileId: profileId,);
    return _dataConnect.mutation("CreateTerminalTheme", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class CreateTerminalThemeTerminalThemeInsert {
  final String id;
  CreateTerminalThemeTerminalThemeInsert.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateTerminalThemeTerminalThemeInsert otherTyped = other as CreateTerminalThemeTerminalThemeInsert;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  CreateTerminalThemeTerminalThemeInsert({
    required this.id,
  });
}

@immutable
class CreateTerminalThemeData {
  final CreateTerminalThemeTerminalThemeInsert terminalTheme_insert;
  CreateTerminalThemeData.fromJson(dynamic json):
  
  terminalTheme_insert = CreateTerminalThemeTerminalThemeInsert.fromJson(json['terminalTheme_insert']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateTerminalThemeData otherTyped = other as CreateTerminalThemeData;
    return terminalTheme_insert == otherTyped.terminalTheme_insert;
    
  }
  @override
  int get hashCode => terminalTheme_insert.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['terminalTheme_insert'] = terminalTheme_insert.toJson();
    return json;
  }

  CreateTerminalThemeData({
    required this.terminalTheme_insert,
  });
}

@immutable
class CreateTerminalThemeVariables {
  final String name;
  final String palette;
  final String profileId;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  CreateTerminalThemeVariables.fromJson(Map<String, dynamic> json):
  
  name = nativeFromJson<String>(json['name']),
  palette = nativeFromJson<String>(json['palette']),
  profileId = nativeFromJson<String>(json['profileId']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateTerminalThemeVariables otherTyped = other as CreateTerminalThemeVariables;
    return name == otherTyped.name && 
    palette == otherTyped.palette && 
    profileId == otherTyped.profileId;
    
  }
  @override
  int get hashCode => Object.hashAll([name.hashCode, palette.hashCode, profileId.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['name'] = nativeToJson<String>(name);
    json['palette'] = nativeToJson<String>(palette);
    json['profileId'] = nativeToJson<String>(profileId);
    return json;
  }

  CreateTerminalThemeVariables({
    required this.name,
    required this.palette,
    required this.profileId,
  });
}

